"use strict";

const crypto = require("node:crypto");

function digest(text) {
  return crypto.createHash("sha256").update(text.replace(/\r\n/g, "\n")).digest("hex");
}

function grammarRules(text) {
  const grammar = text.replace(/\(\*[\s\S]*?\*\)/g, "");
  return [...grammar.matchAll(/^([A-Za-z_][A-Za-z_0-9]*)\s*=/gm)].map((match) => match[1]).sort();
}

function protocolRows(text) {
  const table = text.split("## Protocol Matrix")[1]?.split("## Fixture Design")[0];
  if (!table) throw new Error("protocol matrix headings are missing");
  return table.split(/\r?\n/).filter((line) => line.startsWith("| ") && /\| S\d/.test(line)).map((line) => {
    const columns = line.split("|").slice(1, -1).map((part) => part.trim());
    const [name, capability, dimensions, positive, negative] = columns;
    const syntax = [];
    for (const match of dimensions.matchAll(/S(\d+)(?:-S?(\d+))?/g)) {
      const first = Number(match[1]);
      const last = Number(match[2] ?? match[1]);
      for (let index = first; index <= last; index++) syntax.push(`S${index}`);
    }
    return { name, capability, syntax, positive, negative };
  });
}

function syntaxDimensions(text) {
  return text.split(/\r?\n/).filter((line) => /^\| S\d+ \|/.test(line)).map((line) => {
    const [id, name, surface] = line.split("|").slice(1, -1).map((item) => item.trim());
    return { id, name, surface };
  });
}

function requireUnique(values, label) {
  if (new Set(values).size !== values.length) throw new Error(`duplicate ${label}`);
}

function obligations(catalog) {
  const requirements = [];
  for (const feature of catalog.features) {
    if (!["supported", "unsupported", "conditional"].includes(feature.support)) throw new Error(`invalid support: ${feature.id}`);
    for (const syntax of feature.syntax) {
      for (const polarity of ["positive", "negative"]) {
        // Unsupported features are tested for rejection, never positive behavior.
        if (feature.support === "unsupported" && polarity === "positive") continue;
        for (const layer of feature.layers) {
          requirements.push({ id: `${feature.id}/syntax/${syntax}/${polarity}/${layer}`,
            feature: feature.id, axis: "syntax", value: syntax, polarity, layer });
        }
      }
    }
    for (const axis of ["states", "environments"]) {
      for (const value of feature[axis]) {
        requirements.push({ id: `${feature.id}/${axis}/${value}/protocol`,
          feature: feature.id, axis, value, layer: "protocol" });
      }
    }
    if (feature.editor) requirements.push({ id: `${feature.id}/editor/smoke`,
      feature: feature.id, axis: "editor", value: "smoke", layer: "editor" });
  }
  requireUnique(requirements.map((item) => item.id), "requirement");
  return requirements;
}

function validateCatalog(catalog, sources) {
  if (catalog.version !== 1) throw new Error("unsupported matrix version");
  requireUnique(catalog.features.map((item) => item.id), "feature");
  const rows = protocolRows(sources.protocol);
  if (JSON.stringify(rows) !== JSON.stringify(catalog.protocolBaseline)) {
    throw new Error("protocol requirements changed: review catalog applicability and update protocolBaseline");
  }
  if (JSON.stringify(grammarRules(sources.grammar)) !== JSON.stringify(catalog.grammarRules)) {
    throw new Error("grammar productions changed: classify new/removed rules in the matrix");
  }
  if (JSON.stringify(syntaxDimensions(sources.protocol)) !== JSON.stringify(catalog.syntaxDimensions)) {
    throw new Error("syntax dimension contract changed: review applicability and assertions");
  }
  const classified = catalog.grammarGroups.flatMap((group) => group.productions).sort();
  if (JSON.stringify(classified) !== JSON.stringify(catalog.grammarRules)) {
    throw new Error("every grammar production needs exactly one semantic group");
  }
  for (const group of catalog.grammarGroups) {
    if (!group.syntax.length || group.syntax.some((id) => !catalog.syntaxDimensions.some((dimension) => dimension.id === id))) {
      throw new Error(`unknown syntax dimension in grammar group ${group.id}`);
    }
  }
  for (const [file, hash] of Object.entries(catalog.syntaxSources)) {
    if (digest(sources.syntax[file]) !== hash) {
      throw new Error(`syntax contract changed: review matrix requirements before updating ${file}`);
    }
  }
  const featureRows = catalog.features.map((feature) => feature.row).sort((a, b) => a - b);
  if (JSON.stringify(featureRows) !== JSON.stringify(rows.map((_, index) => index))) {
    throw new Error("each protocol row must have exactly one feature entry");
  }
  for (const feature of catalog.features) {
    if (JSON.stringify(feature.syntax) !== JSON.stringify(rows[feature.row].syntax)) {
      throw new Error(`${feature.id}: syntax applicability differs from the protocol contract`);
    }
    if (!feature.layers.length || !feature.layerRationale || !feature.environmentRationale) {
      throw new Error(`${feature.id}: layers and environment applicability require rationale`);
    }
    for (const layer of feature.layers) {
      if (!["service", "protocol"].includes(layer)) throw new Error(`invalid layer ${layer}`);
    }
    for (const axis of ["states", "environments"]) {
      requireUnique(feature[axis], `${feature.id} ${axis}`);
      for (const item of feature[axis]) {
        if (!catalog[axis][item]) throw new Error(`unknown ${axis}: ${item}`);
      }
    }
  }
  const keys = obligations(catalog).map((item) => item.id);
  requireUnique((catalog.exemptions || []).map((item) => item.requirement), "exemption requirement");
  for (const item of catalog.exemptions || []) {
    if (!keys.includes(item.requirement) || !item.reason?.trim() || catalog.evidence.some((proof) => proof.requirement === item.requirement)) {
      throw new Error(`invalid or conflicting exemption: ${item.requirement}`);
    }
  }
  requireUnique(catalog.evidence.map((item) => item.requirement), "evidence requirement");
  for (const item of catalog.evidence) {
    if (!keys.includes(item.requirement) || !item.assertion || !item.tests?.length) {
      throw new Error(`invalid evidence: ${item.requirement}`);
    }
    const requirement = obligations(catalog).find((row) => row.id === item.requirement);
    for (const test of item.tests) {
      if (test.layer !== requirement.layer || !test.name) throw new Error(`wrong evidence layer: ${item.requirement}`);
    }
  }
  return obligations(catalog);
}

function testNames(output) {
  return [...output.matchAll(/^(.+): test\r?$/gm)].map((match) => match[1]);
}

function testResults(output) {
  return new Map([...output.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)(?:[^\r\n]*)$/gm)]
    .map((match) => [match[1], match[2]]));
}

function assess(requirements, evidence, available, results = {}, exemptions = []) {
  const evidenceById = new Map(evidence.map((item) => [item.requirement, item]));
  return requirements.map((requirement) => {
    const exemption = exemptions.find((item) => item.requirement === requirement.id);
    if (exemption) return { ...requirement, status: "not_applicable", reason: exemption.reason };
    const proof = evidenceById.get(requirement.id);
    if (!proof) return { ...requirement, status: "unreviewed" };
    for (const test of proof.tests) {
      if (!available[test.layer]?.includes(test.name)) throw new Error(`stale test reference: ${test.layer}:${test.name}`);
    }
    const statuses = proof.tests.map((test) => results[test.layer]?.get(test.name));
    const status = statuses.some((state) => state === "FAILED" || state === "ignored") ? "failed"
      : statuses.every((state) => state === "ok") ? "verified" : "mapped";
    return { ...requirement, status, assertion: proof.assertion, tests: proof.tests };
  });
}

module.exports = { digest, grammarRules, protocolRows, syntaxDimensions, obligations, validateCatalog, testNames, testResults, assess };

"use strict";

const { digest } = require("./model");

const batchIds = Array.from({ length: 20 }, (_, index) => `B${String(index).padStart(2, "0")}`);
const deferredScenarios = ["UX19", "UX20", "UX22", "UX23", "UX24"];

function unique(values, label) {
  if (new Set(values).size !== values.length) throw new Error(`duplicate ${label}`);
}

function sameSet(actual, expected, label) {
  unique(actual, label);
  if (JSON.stringify([...actual].sort()) !== JSON.stringify([...expected].sort())) {
    throw new Error(`${label} differs from required inventory`);
  }
}

function tableRows(text, pattern) {
  return text.split(/\r?\n/).filter((line) => pattern.test(line))
    .map((line) => line.split("|").slice(1, -1).map((part) => part.trim()));
}

function batchContracts(plan) {
  const rows = tableRows(plan, /^\| B\d{2} \|/);
  sameSet(rows.map(([id]) => id), batchIds, "batch contract");
  return rows.map(([id, scope, proof]) => ({ id, scope, proof,
    features: [...scope.matchAll(/`([a-z-]+)`/g)].map((match) => match[1]) }));
}

function interactionContracts(text) {
  const rows = tableRows(text, /^\| UX\d{2} \|/);
  sameSet(rows.map(([id]) => id), Array.from({ length: 24 }, (_, index) =>
    `UX${String(index + 1).padStart(2, "0")}`), "interaction scenarios");
  return rows.map(([id, owner, action, oracle]) => ({ id, owner, action, oracle,
    levels: [...new Set(action.match(/\b(?:Input|Command|Render)\b/g))].sort() }));
}

function expandInteractions(scenarios, contracts) {
  sameSet(scenarios.map((scenario) => scenario.id), contracts.map((row) => row.id), "interaction scenarios");
  const requirements = [];
  for (const scenario of scenarios) {
    const contract = contracts.find((row) => row.id === scenario.id);
    if (scenario.owner !== contract.owner || scenario.contractHash !== digest(JSON.stringify(contract))) {
      throw new Error(`${scenario.id}: interaction contract changed; review routes and assertions`);
    }
    const deferred = deferredScenarios.includes(scenario.id);
    if (scenario.scope !== (deferred ? "deferred" : "local") || (scenario.owner === "B16") !== deferred) {
      throw new Error(`${scenario.id}: invalid scope deferral`);
    }
    if (deferred) {
      if (scenario.routes.length || !scenario.reason?.trim()) throw new Error(`${scenario.id}: invalid deferred family`);
      continue;
    }
    if (!scenario.routes.length) throw new Error(`${scenario.id}: missing routes`);
    unique(scenario.routes.map((route) => route.id), `${scenario.id} route`);
    for (const polarity of ["positive", "negative"]) {
      if (!scenario.routes.some((route) => route.polarity === polarity)) {
        throw new Error(`${scenario.id}: missing ${polarity} route`);
      }
    }
    for (const route of scenario.routes) {
      if (!/^[a-z][a-z0-9-]*$/.test(route.id) || !route.assertion?.trim() ||
          !route.action?.trim() || !/^[a-z][a-z0-9-]*$/.test(route.fixture) ||
          !["positive", "negative", "recovery"].includes(route.polarity)) {
        throw new Error(`${scenario.id}: invalid route definition`);
      }
      sameSet(route.levels, contract.levels, `${scenario.id}/${route.id} evidence levels`);
      for (const level of route.levels) {
        requirements.push({ id: `vscode/${scenario.id}/${route.id}/${level.toLowerCase()}/local`,
          owner: scenario.owner, layer: "interaction", scenario: scenario.id, route: route.id,
          level, polarity: route.polarity, fixture: route.fixture,
          contractHash: digest(JSON.stringify({ contract, route })) });
      }
    }
  }
  return requirements;
}

function executionRequirements(catalog, requirements, manifest, scenarios, sources) {
  const contracts = batchContracts(sources.plan);
  const featureOwners = new Map(contracts.flatMap((batch) => batch.features.map((feature) => [feature, batch.id])));
  const declaredFeatures = contracts.flatMap((batch) => batch.features);
  sameSet(declaredFeatures, catalog.features.map((feature) => feature.id), "feature ownership");
  const semantic = requirements.map((requirement) => {
    const feature = catalog.features.find((feature) => feature.id === requirement.feature);
    const exemption = catalog.exemptions.find((item) => item.requirement === requirement.id)?.reason ?? null;
    const contract = { requirement, support: feature.support,
      protocol: catalog.protocolBaseline[feature.row],
      axis: catalog[requirement.axis]?.[requirement.value] ?? null, exemption };
    return { ...requirement, owner: featureOwners.get(requirement.feature),
      contractHash: digest(JSON.stringify(contract)) };
  });
  const interactions = expandInteractions(scenarios, interactionContracts(sources.interactions));
  const deliverables = manifest.batches.flatMap((batch) => batch.deliverables.map((item) => {
    if (!/^[a-z][a-z0-9-]*$/.test(item.id) || !item.assertion?.trim() || !["gate", "review"].includes(item.layer)) {
      throw new Error(`${batch.id}: invalid deliverable`);
    }
    return { id: `batch/${batch.id}/${item.id}`, owner: batch.id, layer: item.layer,
      assertion: item.assertion, contractHash: digest(JSON.stringify(item)) };
  }));
  const all = [...semantic, ...interactions, ...deliverables];
  unique(all.map((item) => item.id), "execution requirement");
  return all;
}

function validateBaseline(requirements, baseline, migrations) {
  if (baseline.version !== 1) throw new Error("unsupported execution baseline version");
  unique(baseline.requirements.map((item) => item.id), "baseline requirement");
  unique(migrations.map((item) => item.id), "migration");
  const current = new Map(requirements.map((item) => [item.id, item]));
  const original = new Map(baseline.requirements.map((item) => [item.id, item]));
  const migrated = new Set();
  for (const migration of migrations) {
    if (!migration.reason?.trim() || !migration.preservedBehavior?.trim() || !migration.from?.length || !migration.to?.length) {
      throw new Error("migration must preserve the original behavior and name exact requirements");
    }
    for (const from of migration.from) {
      const item = original.get(from.id);
      if (!item || item.contractHash !== from.contractHash || migrated.has(from.id)) {
        throw new Error(`invalid migration source ${from.id}`);
      }
      migrated.add(from.id);
    }
    for (const to of migration.to) {
      const item = current.get(to.id);
      if (!item || item.contractHash !== to.contractHash) throw new Error(`invalid migration target ${to.id}`);
    }
  }
  for (const original of baseline.requirements) {
    const item = current.get(original.id);
    if ((!item || item.owner !== original.owner || item.contractHash !== original.contractHash) && !migrated.has(original.id)) {
      throw new Error(`${original.id}: removed or changed requirement requires an explicit migration`);
    }
  }
}

function validateManifest(manifest, baseline, catalog, requirements, scenarios, sources) {
  if (manifest.version !== 1 || manifest.scope !== "local") throw new Error("unsupported execution manifest");
  sameSet(manifest.batches.map((batch) => batch.id), batchIds, "batch manifest");
  for (const batch of manifest.batches) {
    if (batch.scope !== (batch.id === "B16" ? "deferred" : "local")) throw new Error(`${batch.id}: invalid batch deferral`);
    if (batch.id === "B16" && (batch.requirements.length || batch.deliverables.length)) {
      throw new Error("B16 must not own active local requirements");
    }
    if (batch.id !== "B16" && !batch.requirements.length) throw new Error(`${batch.id}: empty batch`);
  }
  const all = executionRequirements(catalog, requirements, manifest, scenarios, sources);
  const owners = new Map();
  for (const batch of manifest.batches) {
    for (const id of batch.requirements) {
      if (owners.has(id)) throw new Error(`duplicate requirement owner: ${id}`);
      owners.set(id, batch.id);
    }
  }
  sameSet([...owners.keys()], all.map((item) => item.id), "owned requirements");
  for (const requirement of all) {
    if (owners.get(requirement.id) !== requirement.owner) throw new Error(`wrong requirement owner: ${requirement.id}`);
  }
  validateBaseline(all, baseline, manifest.migrations);
  return all;
}

module.exports = { batchIds, batchContracts, interactionContracts, expandInteractions,
  executionRequirements, validateBaseline, validateManifest };

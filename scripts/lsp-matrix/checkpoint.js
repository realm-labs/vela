"use strict";

const { digest } = require("./model");
const { batchIds } = require("./execution");

const localOrder = batchIds.filter((id) => id !== "B16");

function requirementScope(requirements, id) {
  return requirements.filter((item) => item.owner === id)
    .map(({ id, contractHash }) => ({ id, contractHash })).sort((a, b) => a.id.localeCompare(b.id));
}

function firstPending(checkpoint) {
  return localOrder.find((id) => !checkpoint.acceptedBatches.some((batch) => batch.id === id)) ?? null;
}

function remaining(checkpoint, requirements) {
  const accepted = new Set(checkpoint.acceptedBatches.map((batch) => batch.id));
  return requirements.filter((item) => !accepted.has(item.owner)).map((item) => item.id).sort();
}

function equal(actual, expected, message) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(message);
}

function validSource(source) {
  return /^[a-f0-9]{40}$/.test(source?.revision) && /^[a-f0-9]{64}$/.test(source?.treeSha256);
}

function validateAcceptance(batch) {
  if (!localOrder.includes(batch.id) || !batch.requirements?.length || !validSource(batch.validation?.source) ||
      !batch.validation.commands?.length || batch.validation.commands.some((command) => !command.command || command.exitCode !== 0) ||
      !batch.validation.profile?.platform || !batch.validation.profile?.arch || !/^\d+\.\d+\.\d+$/.test(batch.validation.profile?.vscodeVersion)) {
    throw new Error(`${batch.id}: invalid accepted batch validation`);
  }
}

function validateCheckpoint(checkpoint, manifest, requirements) {
  if (checkpoint.version !== 1 || checkpoint.scope !== "local" || checkpoint.manifestVersion !== manifest.version) {
    throw new Error("unsupported checkpoint version or scope");
  }
  equal(checkpoint.deferredBatches, ["B16"], "only B16 may be deferred");
  if (!checkpoint.profile?.platform || !checkpoint.profile?.arch || !/^\d+\.\d+\.\d+$/.test(checkpoint.profile?.vscodeVersion)) {
    throw new Error("checkpoint requires an exact local profile");
  }
  const known = new Set();
  for (const batch of [...checkpoint.acceptedBatches, ...checkpoint.reopenedBatches]) {
    if (known.has(batch.id)) throw new Error(`duplicate checkpoint batch ${batch.id}`);
    known.add(batch.id);
    validateAcceptance(batch);
    equal(batch.validation.profile, checkpoint.profile, `${batch.id}: changed local profile requires explicit re-audit`);
  }
  for (const batch of checkpoint.reopenedBatches) {
    if (!batch.reason?.trim()) throw new Error(`${batch.id}: reopening requires a reason`);
  }
  for (const batch of checkpoint.acceptedBatches) {
    equal(batch.requirements, requirementScope(requirements, batch.id), `${batch.id}: accepted scope changed; reopen and re-audit`);
    for (const earlier of localOrder.slice(0, localOrder.indexOf(batch.id))) {
      if (!known.has(earlier)) throw new Error(`${batch.id}: prerequisite ${earlier} was never accepted`);
    }
  }
  const pending = firstPending(checkpoint);
  if (checkpoint.nextTask !== pending && !checkpoint.nextTask?.startsWith(`${pending}.`)) {
    throw new Error("checkpoint next task is not the first incomplete local batch");
  }
  if (checkpoint.activeChild !== null && (!/^B\d{2}\.\d+$/.test(checkpoint.activeChild) ||
      checkpoint.activeChild.split(".")[0] !== pending || checkpoint.nextTask !== checkpoint.activeChild)) {
    throw new Error("checkpoint active child must belong to the next local batch");
  }
  equal(checkpoint.remainingRequirements, remaining(checkpoint, requirements), "checkpoint remaining requirement IDs are stale");
  if (!Array.isArray(checkpoint.completedChildren) || !Array.isArray(checkpoint.artifacts) || !Array.isArray(checkpoint.openIssues)) {
    throw new Error("checkpoint resume metadata is missing");
  }
  const children = new Set();
  for (const child of checkpoint.completedChildren) {
    if (!/^B\d{2}\.\d+$/.test(child.id) || !localOrder.includes(child.id.split(".")[0]) || children.has(child.id) ||
        !child.summary?.trim() || !child.requirements?.length || child.requirements.some((id) => typeof id !== "string" || !id) ||
        (child.revision !== null && !/^[a-f0-9]{7,40}$/.test(child.revision))) {
      throw new Error("invalid completed child checkpoint");
    }
    children.add(child.id);
  }
  if (checkpoint.artifacts.some((file) => typeof file !== "string" || file.startsWith("/") || file.includes("..") || /^[A-Za-z]:/.test(file))) {
    throw new Error("checkpoint artifact paths must be repository-relative");
  }
  return checkpoint;
}

function gate(requirements, assessed, checkpoint, selected, testCommandFailed) {
  if (testCommandFailed || assessed.some((item) => item.status === "failed")) {
    throw new Error("an executed test failed; batch acceptance failed");
  }
  if (selected !== "all" && !localOrder.includes(selected)) throw new Error("unknown or deferred batch selection");
  const required = new Set(selected === "all" ? localOrder : [selected, ...checkpoint.acceptedBatches.map((batch) => batch.id)]);
  const byId = new Map(assessed.map((item) => [item.id, item]));
  if (byId.size !== assessed.length) throw new Error("duplicate assessed requirement");
  const missing = requirements.filter((item) => {
    if (!required.has(item.owner)) return false;
    const proof = byId.get(item.id);
    return proof?.contractHash !== item.contractHash ||
      !(proof.status === "verified" || (item.feature && proof.status === "not_applicable"));
  });
  if (missing.length) throw new Error(`batch acceptance has ${missing.length} unverified obligations: ${missing.slice(0, 8).map((item) => item.id).join(", ")}`);
  if (required.has("B19") && checkpoint.openIssues.length) throw new Error("final acceptance has unresolved checkpoint issues");
  return [...required].sort();
}

function acceptBatch(checkpoint, manifest, requirements, assessed, id, validation, testCommandFailed) {
  validateCheckpoint(checkpoint, manifest, requirements);
  if (firstPending(checkpoint) !== id) throw new Error("acceptance must close the first incomplete local batch");
  gate(requirements, assessed, checkpoint, id, testCommandFailed);
  const accepted = { id, requirements: requirementScope(requirements, id), validation };
  validateAcceptance(accepted);
  const next = structuredClone(checkpoint);
  next.acceptedBatches.push(accepted);
  next.artifacts = [...new Set([...next.artifacts, ...(validation.artifacts ?? [])])];
  next.acceptedBatches.sort((a, b) => a.id.localeCompare(b.id));
  next.reopenedBatches = next.reopenedBatches.filter((batch) => batch.id !== id);
  if (next.activeChild && !next.completedChildren.some((child) => child.id === next.activeChild)) {
    next.completedChildren.push({ id: next.activeChild, revision: null,
      requirements: accepted.requirements.map((item) => item.id),
      summary: `Close ${id} with the recorded strict validation; find this child's commit through its LSP-Batch trailer.` });
  }
  next.activeChild = null;
  next.nextTask = firstPending(next);
  next.remainingRequirements = remaining(next, requirements);
  return validateCheckpoint(next, manifest, requirements);
}

function reopenBatch(checkpoint, requirements, id, reason) {
  if (!reason?.trim()) throw new Error("reopening requires an explicit reason");
  const batch = checkpoint.acceptedBatches.find((batch) => batch.id === id);
  if (!batch) throw new Error("only an accepted batch can be reopened");
  const next = structuredClone(checkpoint);
  next.acceptedBatches = next.acceptedBatches.filter((batch) => batch.id !== id);
  next.reopenedBatches.push({ ...structuredClone(batch), reason });
  next.activeChild = null;
  next.nextTask = firstPending(next);
  next.remainingRequirements = remaining(next, requirements);
  return next;
}

function manifestIdentity(manifest) {
  return digest(JSON.stringify(manifest));
}

function validateExecutionProfile(expected, actual, scoped) {
  // Existing multi-platform smoke CI is an ordinary audit, not acceptance of
  // this local goal. Pin the machine/editor only when invoking a strict gate.
  if (scoped && (expected.platform !== actual.platform || expected.arch !== actual.arch ||
      (actual.vscodeVersion !== undefined && expected.vscodeVersion !== actual.vscodeVersion))) {
    throw new Error("evidence does not match the recorded local execution profile");
  }
}

module.exports = { localOrder, requirementScope, firstPending, remaining, validSource,
  validateCheckpoint, gate, acceptBatch, reopenBatch, manifestIdentity, validateExecutionProfile };

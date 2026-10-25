"use strict";

// Installed sessions do not own the backend's SourceIds. Keep every schema4
// value fact, but never invent source locations from the authored marker names.
function metadataArtifact(schema) {
  const facts = structuredClone(schema);
  for (const entries of Object.values(facts)) for (const entry of entries) delete entry.sourceSpan;
  return { formatVersion: 4, facts };
}

module.exports = { metadataArtifact };

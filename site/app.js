import init, { compile_script, run_script } from "./pkg/vela_playground_wasm.js";

const docs = {
  en: ["overview", "quickstart", "host-bridge", "hot-reload"],
  zh: ["overview", "quickstart", "host-bridge", "hot-reload"],
};

const docView = document.querySelector("#doc-view");
const playgroundView = document.querySelector("#playground-view");
const exampleSelect = document.querySelector("#example-select");
const sourceEditor = document.querySelector("#source-editor");
const entryInput = document.querySelector("#entry-input");
const outputView = document.querySelector("#output-view");
const diagnosticList = document.querySelector("#diagnostic-list");
const runtimeStatus = document.querySelector("#runtime-status");
const examples = window.VELA_PLAYGROUND_EXAMPLES || [];

let wasmReady = false;

for (const [index, example] of examples.entries()) {
  const option = document.createElement("option");
  option.value = String(index);
  option.textContent = example.title;
  exampleSelect.append(option);
}

function loadExample(index) {
  const example = examples[index] || examples[0];
  if (!example) {
    return;
  }
  sourceEditor.value = example.source;
  entryInput.value = example.entry;
  outputView.textContent = "";
  diagnosticList.replaceChildren();
}

exampleSelect.addEventListener("change", () => loadExample(Number(exampleSelect.value)));
document.querySelector("#compile-button").addEventListener("click", () => execute("compile"));
document.querySelector("#run-button").addEventListener("click", () => execute("run"));
window.addEventListener("hashchange", route);

async function boot() {
  loadExample(0);
  try {
    await init();
    wasmReady = true;
    runtimeStatus.textContent = "ready";
  } catch (error) {
    runtimeStatus.textContent = "wasm unavailable";
    outputView.textContent = String(error);
  }
  route();
}

function route() {
  const hash = window.location.hash || "#/en/overview";
  const [, first, second] = hash.split("/");
  document.querySelectorAll(".nav-group a").forEach((link) => {
    link.classList.toggle("active", link.getAttribute("href") === hash);
  });

  if (first === "playground") {
    docView.hidden = true;
    playgroundView.hidden = false;
    return;
  }

  const lang = docs[first] ? first : "en";
  const slug = docs[lang].includes(second) ? second : "overview";
  docView.hidden = false;
  playgroundView.hidden = true;
  renderDoc(lang, slug);
}

async function renderDoc(lang, slug) {
  try {
    const response = await fetch(`./docs/${lang}/${slug}.md`);
    if (!response.ok) {
      throw new Error(`missing document: ${lang}/${slug}`);
    }
    docView.innerHTML = markdownToHtml(await response.text());
  } catch (error) {
    docView.innerHTML = `<h1>Document unavailable</h1><p>${escapeHtml(String(error))}</p>`;
  }
}

function execute(mode) {
  if (!wasmReady) {
    runtimeStatus.textContent = "wasm still loading";
    return;
  }

  const responseText =
    mode === "compile"
      ? compile_script(sourceEditor.value)
      : run_script(sourceEditor.value, entryInput.value || "main");
  const response = JSON.parse(responseText);
  runtimeStatus.textContent = response.ok ? "ok" : "error";
  outputView.textContent = response.ok
    ? JSON.stringify(response.value, null, 2)
    : "No value returned.";
  renderDiagnostics(response.diagnostics || []);
}

function renderDiagnostics(diagnostics) {
  diagnosticList.replaceChildren();
  for (const diagnostic of diagnostics) {
    const item = document.createElement("div");
    item.className = `diagnostic ${diagnostic.severity === "warning" ? "warning" : ""}`;
    const code = diagnostic.code ? ` [${diagnostic.code}]` : "";
    const span = diagnostic.span
      ? ` at ${diagnostic.span.start}..${diagnostic.span.end}`
      : "";
    item.textContent = `${diagnostic.severity}${code}: ${diagnostic.message}${span}`;
    diagnosticList.append(item);
  }
}

function markdownToHtml(markdown) {
  const lines = markdown.replace(/\r\n/g, "\n").split("\n");
  const html = [];
  let inCode = false;
  let codeLanguage = "";
  let codeLines = [];
  let inList = false;

  for (const line of lines) {
    if (line.startsWith("```")) {
      if (inCode) {
        html.push(renderCodeBlock(codeLines.join("\n"), codeLanguage));
        inCode = false;
        codeLanguage = "";
        codeLines = [];
      } else {
        closeList();
        inCode = true;
        codeLanguage = line.slice(3).trim();
      }
      continue;
    }

    if (inCode) {
      codeLines.push(line);
      continue;
    }

    if (line.startsWith("# ")) {
      closeList();
      html.push(`<h1>${inlineMarkdown(line.slice(2))}</h1>`);
    } else if (line.startsWith("## ")) {
      closeList();
      html.push(`<h2>${inlineMarkdown(line.slice(3))}</h2>`);
    } else if (line.startsWith("### ")) {
      closeList();
      html.push(`<h3>${inlineMarkdown(line.slice(4))}</h3>`);
    } else if (line.startsWith("- ")) {
      if (!inList) {
        html.push("<ul>");
        inList = true;
      }
      html.push(`<li>${inlineMarkdown(line.slice(2))}</li>`);
    } else if (line.trim() === "") {
      closeList();
    } else {
      closeList();
      html.push(`<p>${inlineMarkdown(line)}</p>`);
    }
  }

  closeList();
  if (inCode) {
    html.push(renderCodeBlock(codeLines.join("\n"), codeLanguage));
  }
  return html.join("");

  function closeList() {
    if (inList) {
      html.push("</ul>");
      inList = false;
    }
  }
}

function renderCodeBlock(code, language) {
  const lang = language || "text";
  const highlighted = lang === "vela" ? highlightVela(code) : escapeHtml(code);
  return `
    <figure class="code-card">
      <figcaption><span>${escapeHtml(lang)}</span></figcaption>
      <pre><code class="language-${escapeHtml(lang)}">${highlighted}</code></pre>
    </figure>`;
}

function highlightVela(code) {
  const placeholders = [];
  let escaped = escapeHtml(code);
  escaped = escaped.replace(/(&quot;(?:\\.|[^&])*?&quot;)/g, (match) =>
    stash(`<span class="tok-string">${match}</span>`),
  );
  escaped = escaped.replace(/\b(\d+(?:\.\d+)?)\b/g, '<span class="tok-number">$1</span>');
  escaped = escaped.replace(
    /\b(struct|enum|trait|impl|for|fn|let|return|if|else|match|true|false|null|global)\b/g,
    '<span class="tok-keyword">$1</span>',
  );
  escaped = escaped.replace(
    /\b(Int|Float|Bool|String|None|Option|Result)\b/g,
    '<span class="tok-type">$1</span>',
  );
  escaped = escaped.replace(/\b([a-zA-Z_][\w]*)\s*(?=\()/g, '<span class="tok-call">$1</span>');
  return placeholders.reduce(
    (text, value, index) => text.replace(`__VELA_TOKEN_${index}__`, value),
    escaped,
  );

  function stash(value) {
    const index = placeholders.length;
    placeholders.push(value);
    return `__VELA_TOKEN_${index}__`;
  }
}

function inlineMarkdown(text) {
  return escapeHtml(text)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
}

function escapeHtml(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

boot();

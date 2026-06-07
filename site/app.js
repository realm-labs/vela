import init, { compile_script, run_script } from "./pkg/vela_playground_wasm.js";

const docs = {
  en: {
    label: "English",
    fallback: "overview",
    groups: [
      {
        title: "Guide",
        pages: [
          ["overview", "Overview"],
          ["quickstart", "Quickstart"],
          ["playground", "Playground"],
        ],
      },
      {
        title: "Language",
        pages: [
          ["language/syntax", "Syntax"],
          ["language/types", "Types And Values"],
          ["language/control-flow", "Control Flow"],
          ["language/functions-methods", "Functions And Methods"],
          ["language/collections", "Collections"],
          ["language/modules", "Modules"],
        ],
      },
      {
        title: "Embedding",
        pages: [
          ["embedding/runtime", "Runtime API"],
          ["embedding/native-functions", "Native Functions"],
          ["embedding/host-bridge", "Host Bridge"],
          ["embedding/globals-serde", "Globals And Serde"],
          ["reflection", "Reflection"],
          ["hot-reload", "Hot Reload"],
        ],
      },
    ],
  },
  zh: {
    label: "中文",
    fallback: "overview",
    groups: [
      {
        title: "指南",
        pages: [
          ["overview", "概览"],
          ["quickstart", "快速开始"],
          ["playground", "Playground"],
        ],
      },
      {
        title: "语言",
        pages: [
          ["language/syntax", "语法"],
          ["language/types", "类型和值"],
          ["language/control-flow", "控制流"],
          ["language/functions-methods", "函数和方法"],
          ["language/collections", "集合"],
          ["language/modules", "模块"],
        ],
      },
      {
        title: "嵌入",
        pages: [
          ["embedding/runtime", "Runtime API"],
          ["embedding/native-functions", "Native 函数"],
          ["embedding/host-bridge", "Host 边界"],
          ["embedding/globals-serde", "Global 和 Serde"],
          ["reflection", "反射"],
          ["hot-reload", "热更新"],
        ],
      },
    ],
  },
};

const docView = document.querySelector("#doc-view");
const playgroundView = document.querySelector("#playground-view");
const sidebar = document.querySelector("#sidebar");
const langEn = document.querySelector("#lang-en");
const langZh = document.querySelector("#lang-zh");
const docsLink = document.querySelector('[data-i18n="docs"]');
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
  const [, first, ...slugParts] = hash.split("/");
  const currentSlug = slugParts.join("/") || "overview";

  if (first === "playground") {
    docView.hidden = true;
    playgroundView.hidden = false;
    renderSidebar("en", "playground");
    updateLanguageLinks("en", "playground");
    return;
  }

  const lang = docs[first] ? first : "en";
  const slug = pageExists(lang, currentSlug) ? currentSlug : docs[lang].fallback;
  docView.hidden = false;
  playgroundView.hidden = true;
  renderSidebar(lang, slug);
  updateLanguageLinks(lang, slug);
  renderDoc(lang, slug);
}

function pageExists(lang, slug) {
  return docs[lang].groups.some((group) => group.pages.some(([page]) => page === slug));
}

function renderSidebar(lang, activeSlug) {
  const current = docs[lang];
  const fragment = document.createDocumentFragment();
  for (const group of current.groups) {
    const section = document.createElement("div");
    section.className = "nav-group";
    const title = document.createElement("span");
    title.textContent = group.title;
    section.append(title);
    for (const [slug, label] of group.pages) {
      const link = document.createElement("a");
      link.href = slug === "playground" ? "#/playground" : `#/${lang}/${slug}`;
      link.textContent = label;
      link.classList.toggle("active", slug === activeSlug);
      section.append(link);
    }
    fragment.append(section);
  }
  sidebar.replaceChildren(fragment);
}

function updateLanguageLinks(lang, slug) {
  const targetSlug = slug === "playground" ? "overview" : slug;
  langEn.href = `#/en/${pageExists("en", targetSlug) ? targetSlug : docs.en.fallback}`;
  langZh.href = `#/zh/${pageExists("zh", targetSlug) ? targetSlug : docs.zh.fallback}`;
  docsLink.href = `#/${lang}/${docs[lang].fallback}`;
  langEn.classList.toggle("active", lang === "en");
  langZh.classList.toggle("active", lang === "zh");
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
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
  let listType = null;

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
      openList("ul");
      html.push(`<li>${inlineMarkdown(line.slice(2))}</li>`);
    } else if (/^\d+\.\s+/.test(line)) {
      openList("ol");
      html.push(`<li>${inlineMarkdown(line.replace(/^\d+\.\s+/, ""))}</li>`);
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
    if (listType) {
      html.push(`</${listType}>`);
      listType = null;
    }
  }

  function openList(type) {
    if (listType === type) {
      return;
    }
    closeList();
    html.push(`<${type}>`);
    listType = type;
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

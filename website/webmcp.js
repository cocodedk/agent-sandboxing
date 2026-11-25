// WebMCP tools and code-block copy buttons for the agent-sandboxing site.
// Every answer is read from the page at call time, so this one file serves every language.
// Nothing here makes a network request. API: document.modelContext.registerTool (a moving draft).

const PAGE = 'agent-sandboxing';
const SOURCE_URL = 'https://github.com/cocodedk/agent-sandboxing';
const MAX_TEXT_CHARS = 20000;
const MAX_ID_CHARS = 128;
const COPIED_RESET_MS = 2000;
const COPIED_STATE = 'copied';
const DEFAULT_LANG = 'en';
const DEFAULT_UI = { copy: 'Copy', copied: 'Copied' };
const SEL = {
  section: 'main article section[id]',
  heading: 'h2',
  lead: '.lead',
  languages: 'link[rel="alternate"][hreflang]',
  steps: '#getting-started ol.steps',
  code: '.code pre code',
  block: '.code',
};
const X_DEFAULT = 'x-default';
const NO_INPUT = { type: 'object', properties: {}, additionalProperties: false };
const READ_ONLY = { readOnlyHint: true };
const NAMES = ['describe', 'list_sections', 'get_section', 'list_languages', 'get_install_steps', 'copy_command'];

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
const text = (el) => (el ? el.textContent.replace(/\s+/g, ' ').trim() : '');
const cap = (s, n) => Array.from(s).slice(0, n).join('');
const given = (input) => (input && typeof input === 'object' && !Array.isArray(input) ? input : {});
const lang = () => document.documentElement.lang || DEFAULT_LANG;
const ui = (key) => document.body?.dataset['ui' + key[0].toUpperCase() + key.slice(1)] || DEFAULT_UI[key];

const sections = () => $$(SEL.section).map((s, i) => ({ id: s.id, heading: text($(SEL.heading, s)), index: i + 1 }));
const commands = (root = document) => $$(SEL.code, root).map((c) => c.textContent.replace(/\n+$/, ''));

// Text of an element without the copy buttons this file adds.
function plain(el) {
  const copy = el.cloneNode(true);
  copy.querySelectorAll('.copy-btn').forEach((b) => b.remove());
  return text(copy);
}

// The page's translations, read from its <link rel="alternate" hreflang> tags (the brand header shows the same).
const langName = (code) => { try { return new Intl.DisplayNames([code], { type: 'language' }).of(code); } catch { return code; } };
function languages() {
  return $$(SEL.languages).filter((l) => l.hreflang !== X_DEFAULT).map((l) => ({
    code: l.hreflang, name: langName(l.hreflang), url: l.href, current: l.hreflang === lang(),
  }));
}

function stepLists(sectionId) {
  return $$(SEL.steps)
    .map((ol) => ({ ol, section: ol.closest('section[id]') }))
    .filter(({ section }) => !sectionId || section?.id === sectionId)
    .map(({ ol, section }) => ({
      section_id: section?.id ?? '',
      heading: text(section && $(SEL.heading, section)),
      steps: $$(':scope > li', ol).map((li, i) => ({ n: i + 1, text: plain(li), commands: commands(li) })),
    }));
}

// Copies text the way a person's click does; reports what really happened.
const copyText = (value) => Promise.resolve()
  .then(() => navigator.clipboard.writeText(value)).then(() => true, () => false);

const sectionId = (input) => (typeof given(input).id === 'string' ? cap(given(input).id.trim(), MAX_ID_CHARS) : '');

function tools() {
  return [
    {
      name: 'describe',
      title: 'What is on this page',
      description: 'Returns what this agent-sandboxing guide is, its language, its title and lead, its sections and the other tools, in the page language.',
      inputSchema: NO_INPUT,
      annotations: READ_ONLY,
      execute: async () => ({
        ok: true, page: PAGE, source: SOURCE_URL, language: lang(),
        title: text($('h1')), lead: text($(SEL.lead)),
        sections: sections().map(({ id, heading }) => ({ id, heading })),
        languages: languages().map((l) => l.code), tools: NAMES,
      }),
    },
    {
      name: 'list_sections',
      title: 'Sections',
      description: 'Lists the sections of this guide in reading order, each with its id (for get_section), heading and position.',
      inputSchema: NO_INPUT,
      annotations: READ_ONLY,
      execute: async () => ({ ok: true, language: lang(), sections: sections() }),
    },
    {
      name: 'get_section',
      title: 'Read one section',
      description: 'Returns the full text and the code blocks of one section, by the id list_sections gives; an unknown id returns the valid ids.',
      inputSchema: {
        type: 'object',
        properties: { id: { type: 'string', description: 'Section id from list_sections.' } },
        required: ['id'],
        additionalProperties: false,
      },
      annotations: READ_ONLY,
      execute: async (input) => {
        const id = sectionId(input);
        const known = sections().map((s) => s.id);
        if (!known.includes(id)) return { ok: false, error: 'unknown section id', valid_ids: known };
        const el = document.getElementById(id);
        const full = plain(el);
        return {
          ok: true, id, heading: text($(SEL.heading, el)), text: cap(full, MAX_TEXT_CHARS),
          truncated: Array.from(full).length > MAX_TEXT_CHARS, commands: commands(el),
        };
      },
    },
    {
      name: 'list_languages',
      title: 'Languages',
      description: 'Lists the languages this guide is published in, each with its code, its name and the URL of that version.',
      inputSchema: NO_INPUT,
      annotations: READ_ONLY,
      execute: async () => ({ ok: true, current: lang(), languages: languages() }),
    },
    {
      name: 'get_install_steps',
      title: 'Numbered steps',
      description: 'Returns the numbered steps of the guide grouped by section, each step with its text and the commands it shows; it runs nothing. Optionally limited to one section id.',
      inputSchema: {
        type: 'object',
        properties: { id: { type: 'string', description: 'Optional section id to limit the answer to.' } },
        additionalProperties: false,
      },
      annotations: READ_ONLY,
      execute: async (input) => {
        const id = sectionId(input);
        const known = sections().map((s) => s.id);
        if (id && !known.includes(id)) return { ok: false, error: 'unknown section id', valid_ids: known };
        return { ok: true, language: lang(), lists: stepLists(id) };
      },
    },
    {
      name: 'copy_command',
      title: 'Copy a command',
      description: 'Puts the code block at the given 1-based position on the page on the clipboard and returns its text; it runs nothing, and copied is false when the browser refused clipboard access.',
      inputSchema: {
        type: 'object',
        properties: { index: { type: 'integer', minimum: 1, description: '1-based position of the code block on the page.' } },
        required: ['index'],
        additionalProperties: false,
      },
      execute: async (input) => {
        const index = given(input).index;
        const all = commands();
        if (!Number.isInteger(index) || index < 1 || index > all.length) {
          return { ok: false, error: 'index must be an integer from 1 to count', count: all.length };
        }
        const command = all[index - 1];
        return { ok: true, index, command, copied: await copyText(command) };
      },
    },
  ];
}

// One copy button per code block, labelled in the page language from <body data-ui-copy data-ui-copied>.
export function addCopyButtons() {
  $$(SEL.block).forEach((block) => {
    const code = $('pre code', block);
    if (!code || $('.copy-btn', block)) return;
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'copy-btn';
    btn.textContent = ui('copy');
    btn.addEventListener('click', async () => {
      if (!(await copyText(code.textContent.replace(/\n+$/, '')))) return;
      btn.textContent = ui('copied');
      btn.dataset.state = COPIED_STATE;
      setTimeout(() => { btn.textContent = ui('copy'); delete btn.dataset.state; }, COPIED_RESET_MS);
    });
    block.append(btn);
  });
}

// Resolves to the names the browser really holds. Fired and forgotten, so it never rejects.
export async function initWebMcp() {
  const registry = document.modelContext;
  if (!registry || typeof registry.registerTool !== 'function') return [];
  await Promise.all(tools().map((tool) => Promise.resolve()
    .then(() => registry.registerTool(tool))
    .catch(() => { /* one refused tool must not take the others with it */ })));
  const held = await Promise.resolve().then(() => registry.getTools()).catch(() => []);
  return (Array.isArray(held) ? held : []).map((t) => t && t.name).filter((n) => NAMES.includes(n));
}

addCopyButtons();
initWebMcp();

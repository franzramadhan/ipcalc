const $ = (id) => document.getElementById(id);
const ME_NOTE = "Note: the Address below is my public IP. Remove it before sharing this prompt.\n\n";
const TABS = ["explain", "inside", "summarize", "ipv6"];
let resultTab = null; // the tab whose result is on screen

function show(tab) {
  if (!TABS.includes(tab)) tab = "explain";
  for (const t of TABS) {
    $(t).hidden = t !== tab;
    $(`tab-${t}`).setAttribute("aria-selected", t === tab);
  }
  $("out").hidden = resultTab !== tab;
}

function go(tab, params) {
  history.replaceState(null, "", `${params ? `?${params}` : location.pathname}#${tab}`);
  resultTab = tab;
  show(tab);
}

function cell(tag, cls, text) {
  const el = document.createElement(tag);
  el.className = cls;
  el.textContent = text;
  return el;
}

// Label | value, with the "why" in small text under the value.
function factRow(f) {
  const tr = document.createElement("tr");
  const th = cell("th", "l", f.label);
  th.scope = "row";
  const td = document.createElement("td");
  td.append(cell("div", /[a-z]{3,} [a-z]/i.test(f.value) ? "v prose" : "v", f.value), cell("div", "w", f.why));
  tr.append(th, td);
  return tr;
}

// The one line that answers the question, above the details.
function verdict(data) {
  const get = (label) => data.facts.filter((f) => f.label.endsWith(label)).map((f) => f.value);
  const result = get("Result")[0];
  if (result) return { big: result === "inside" ? "✓ Inside" : "✗ Outside", sub: data.title, tone: result };
  const exact = get("Exact summary");
  if (exact.length) {
    const sup = get("Single supernet").join(", ");
    const extra = get("Extra addresses in the supernet").join(", ");
    return { big: exact.join(", "), sub: `Exact summary. One route instead: ${sup} (${extra} extra addresses)`, tone: "" };
  }
  return { big: data.title, sub: get("Type")[0] || data.kind || "", tone: "" };
}

async function run(url, note = "") {
  $("err").hidden = true;
  $("result").hidden = true;
  let data;
  try {
    const res = await fetch(url);
    data = await res.json();
    if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  } catch (e) {
    $("err").textContent = e.message;
    $("err").hidden = false;
    return;
  }
  const v = verdict(data);
  $("big").textContent = v.big;
  $("sub").textContent = v.sub;
  $("verdict").dataset.tone = v.tone;
  $("facts").replaceChildren(...data.facts.map(factRow));

  const sp = data.split;
  $("splitbox").hidden = !sp;
  if (sp) {
    $("splittitle").textContent = `${sp.block} split into /${sp.prefix}: ${sp.total} subnets`;
    $("splitnote").textContent =
      sp.subnets.length < Number(sp.total.replaceAll(",", "")) || sp.total.includes("^")
        ? `Showing the first ${sp.subnets.length}.`
        : "";
    $("subnets").replaceChildren(
      ...sp.subnets.map((s) => {
        const tr = document.createElement("tr");
        tr.append(cell("td", "v", s.network), cell("td", "r", s.range));
        return tr;
      }),
    );
  }

  $("prompt").value = note + data.prompt;
  $("copied").textContent = "";
  $("result").hidden = false;
}

function explain(q, split = "") {
  $("q").value = q;
  $("split").value = split;
  const params = new URLSearchParams({ q });
  if (split) params.set("split", split);
  go("explain", params);
  run(`/api/v1/calc?${params}`);
}

function contains(net, ip) {
  $("net").value = net;
  $("ip").value = ip;
  const params = new URLSearchParams({ net, ip });
  go("inside", params);
  run(`/api/v1/contains?${params}`);
}

function summarize(list) {
  $("sum").value = list;
  go("summarize", new URLSearchParams({ sum: list }));
  run(`/api/v1/summarize?${new URLSearchParams({ q: list })}`);
}

const submit = (id, fn) =>
  $(id).addEventListener("submit", (e) => {
    e.preventDefault();
    fn();
  });
submit("explain", () => explain($("q").value.trim(), $("split").value.trim()));
submit("inside", () => contains($("net").value.trim(), $("ip").value.trim()));
submit("summarize", () => summarize($("sum").value.trim()));
// Ctrl/Cmd+Enter submits the textarea
$("sum").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) $("summarize").requestSubmit();
});
$("me").addEventListener("click", () => {
  go("explain");
  run("/api/v1/me", ME_NOTE);
});
for (const b of document.querySelectorAll("button.ex")) {
  const d = b.dataset;
  b.addEventListener("click", () =>
    d.net ? contains(d.net, d.ip) : d.sum ? summarize(d.sum) : explain(d.q || b.textContent, d.split || ""),
  );
}
$("copy").addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText($("prompt").value);
    $("copied").textContent = "Copied.";
  } catch {
    $("prompt").select();
    $("copied").textContent = "Press Ctrl+C (or Cmd+C) to copy.";
  }
});
for (const t of TABS) {
  $(`tab-${t}`).addEventListener("click", (e) => {
    e.preventDefault(); // no jump to the anchor
    history.replaceState(null, "", `${location.search}#${t}`);
    show(t);
  });
}

const params = new URLSearchParams(location.search);
if (params.get("q")) explain(params.get("q"), params.get("split") || "");
else if (params.get("net")) contains(params.get("net"), params.get("ip") || "");
else if (params.get("sum")) summarize(params.get("sum"));
else show(location.hash.slice(1));

const $ = (id) => document.getElementById(id);
const ME_NOTE = "Note: the Address below is my public IP. Remove it before sharing this prompt.\n\n";

function cells(values, classes = []) {
  const tr = document.createElement("tr");
  values.forEach((text, i) => {
    const td = document.createElement("td");
    let cls = classes[i] || "";
    if (cls === "v" && /[a-z]{3,} [a-z]/i.test(text)) cls = "v prose";
    td.className = cls;
    td.textContent = text;
    tr.append(td);
  });
  return tr;
}

async function run(url, note = "") {
  $("err").hidden = true;
  let data;
  try {
    const res = await fetch(url);
    data = await res.json();
    if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`);
  } catch (e) {
    $("err").textContent = e.message;
    $("err").hidden = false;
    $("result").hidden = $("ai").hidden = true;
    return;
  }
  $("title").textContent = data.title;
  $("facts").replaceChildren(...data.facts.map((f) => cells([f.label, f.value, f.why], ["l", "v", "w"])));

  const sp = data.split;
  $("splitbox").hidden = !sp;
  if (sp) {
    $("splittitle").textContent = `${sp.block} split into /${sp.prefix}: ${sp.total} subnets`;
    $("splitnote").textContent =
      sp.subnets.length < Number(sp.total.replaceAll(",", "")) || sp.total.includes("^")
        ? `Showing the first ${sp.subnets.length}.`
        : "";
    $("subnets").replaceChildren(...sp.subnets.map((s) => cells([s.network, s.range], ["v", "r"])));
  }

  $("prompt").value = note + data.prompt;
  $("copied").textContent = "";
  $("result").hidden = $("ai").hidden = false;
}

function explain(q, split = "") {
  $("q").value = q;
  $("split").value = split;
  const params = new URLSearchParams({ q });
  if (split) params.set("split", split);
  history.replaceState(null, "", `?${params}`);
  run(`/api/v1/calc?${params}`);
}

function contains(net, ip) {
  $("net").value = net;
  $("ip").value = ip;
  const params = new URLSearchParams({ net, ip });
  history.replaceState(null, "", `?${params}`);
  run(`/api/v1/contains?${params}`);
}

function summarize(list) {
  $("sum").value = list;
  history.replaceState(null, "", `?${new URLSearchParams({ sum: list })}`);
  run(`/api/v1/summarize?${new URLSearchParams({ q: list })}`);
}

$("form").addEventListener("submit", (e) => {
  e.preventDefault();
  explain($("q").value.trim(), $("split").value.trim());
});
$("cform").addEventListener("submit", (e) => {
  e.preventDefault();
  contains($("net").value.trim(), $("ip").value.trim());
});
$("sform").addEventListener("submit", (e) => {
  e.preventDefault();
  summarize($("sum").value.trim());
});
$("me").addEventListener("click", () => {
  history.replaceState(null, "", "/");
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

const params = new URLSearchParams(location.search);
if (params.get("q")) explain(params.get("q"), params.get("split") || "");
else if (params.get("net")) contains(params.get("net"), params.get("ip") || "");
else if (params.get("sum")) summarize(params.get("sum"));

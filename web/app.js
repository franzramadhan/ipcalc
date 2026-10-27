const $ = (id) => document.getElementById(id);
const ME_NOTE = "Note: the Address below is my public IP. Remove it before sharing this prompt.\n\n";

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
  $("title").textContent = `${data.canonical} (${data.family === "ipv4" ? "IPv4" : "IPv6"})`;
  const rows = data.facts.map((f) => {
    const tr = document.createElement("tr");
    for (const [cls, text] of [["l", f.label], ["v", f.value], ["w", f.why]]) {
      const td = document.createElement("td");
      td.className = cls === "v" && /\s/.test(text) && text.length > 30 ? "v prose" : cls;
      td.textContent = text;
      tr.append(td);
    }
    return tr;
  });
  $("facts").replaceChildren(...rows);
  $("prompt").value = note + data.prompt;
  $("copied").textContent = "";
  $("result").hidden = $("ai").hidden = false;
}

function explain(q) {
  $("q").value = q;
  history.replaceState(null, "", `?q=${encodeURIComponent(q)}`);
  run(`/api/v1/calc?q=${encodeURIComponent(q)}`);
}

$("form").addEventListener("submit", (e) => {
  e.preventDefault();
  explain($("q").value.trim());
});
$("me").addEventListener("click", () => {
  history.replaceState(null, "", "/");
  run("/api/v1/me", ME_NOTE);
});
for (const b of document.querySelectorAll("button.ex")) {
  b.addEventListener("click", () => explain(b.textContent));
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

const q = new URLSearchParams(location.search).get("q");
if (q) explain(q);

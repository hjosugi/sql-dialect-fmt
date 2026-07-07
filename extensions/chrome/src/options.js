const DEFAULT_OPTIONS = {
  lineWidth: 100,
  indentWidth: 4,
  uppercaseKeywords: true,
  dialect: "snowflake"
};

const form = document.querySelector("#options-form");
const status = document.querySelector("#status");

restoreOptions();
form.addEventListener("submit", saveOptions);

async function restoreOptions() {
  const options = await chrome.storage.sync.get(DEFAULT_OPTIONS);
  form.elements.dialect.value = options.dialect === "databricks" ? "databricks" : "snowflake";
  form.elements.lineWidth.value = normalizeInteger(options.lineWidth, DEFAULT_OPTIONS.lineWidth);
  form.elements.indentWidth.value = normalizeInteger(options.indentWidth, DEFAULT_OPTIONS.indentWidth);
  form.elements.uppercaseKeywords.checked = options.uppercaseKeywords !== false;
}

async function saveOptions(event) {
  event.preventDefault();
  const options = {
    dialect: form.elements.dialect.value === "databricks" ? "databricks" : "snowflake",
    lineWidth: normalizeInteger(form.elements.lineWidth.value, DEFAULT_OPTIONS.lineWidth),
    indentWidth: normalizeInteger(form.elements.indentWidth.value, DEFAULT_OPTIONS.indentWidth),
    uppercaseKeywords: form.elements.uppercaseKeywords.checked
  };
  await chrome.storage.sync.set(options);
  status.textContent = "Saved.";
  window.setTimeout(() => {
    status.textContent = "";
  }, 1800);
}

function normalizeInteger(value, fallback) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0) {
    return fallback;
  }
  return number;
}

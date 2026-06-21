-- Case 016: SQL, JavaScript, and Python UDF/UDTF definitions in one file
CREATE OR REPLACE FUNCTION UTIL.FN_JSON_LABELS(P_PAYLOAD VARIANT) RETURNS TABLE(label_key STRING,label_value STRING,language_code STRING,confidence FLOAT) LANGUAGE SQL AS
$$
    SELECT
        f.key::STRING AS label_key,
        f.value:value::STRING AS label_value,
        COALESCE(f.value:lang::STRING, 'und') AS language_code,
        TRY_TO_DOUBLE(f.value:confidence::STRING) AS confidence
    FROM TABLE(FLATTEN(INPUT => P_PAYLOAD:labels, OUTER => TRUE)) AS f
    WHERE f.key IS NOT NULL
$$; CREATE OR REPLACE FUNCTION UTIL.FN_NORMALIZE_PHONE(P_VALUE STRING) RETURNS STRING LANGUAGE JAVASCRIPT AS
$$
if (P_VALUE === null) {
    return null;
}
const digits = String(P_VALUE).replace(/[^0-9+]/g, "");
if (digits.startsWith("+")) {
    return digits;
}
if (digits.startsWith("81")) {
    return "+" + digits;
}
if (digits.startsWith("0")) {
    return "+81" + digits.substring(1);
}
return digits;
$$; CREATE OR REPLACE FUNCTION UTIL.FN_SAFE_SLUG(P_VALUE STRING) RETURNS STRING LANGUAGE PYTHON RUNTIME_VERSION = '3.12' HANDLER = 'slugify' AS
$$
import re
import unicodedata

def slugify(value):
    if value is None:
        return None
    text = unicodedata.normalize("NFKC", str(value)).lower()
    text = re.sub(r"[^a-z0-9]+", "-", text)
    text = re.sub(r"^-+|-+$", "", text)
    return text or None
$$;

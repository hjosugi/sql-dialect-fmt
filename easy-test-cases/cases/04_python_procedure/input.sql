-- Case 04: Python Snowpark stored procedure with Unicode profiling.
create or replace procedure OPS.SP_PY_PROFILE_MULTILINGUAL(P_SOURCE_TABLE VARCHAR, P_TEXT_COLUMN VARCHAR, P_LIMIT INTEGER default 1000) Returns Variant Language PYTHON RUNTIME_VERSION = '3.12' PACKAGES = ('snowflake-snowpark-python') HANDLER = 'main' Execute As Caller As $$
import re,unicodedata
from collections import Counter
from typing import Any
from snowflake.snowpark import Session
from snowflake.snowpark.functions import col
SCRIPT_PATTERNS={"hiragana":re.compile(r"[ぁ-ゟ]"),"katakana":re.compile(r"[ァ-ヿ]"),"han":re.compile(r"[一-鿿]"),"hangul":re.compile(r"[가-힣]"),"arabic":re.compile(r"[؀-ۿ]"),"hebrew":re.compile(r"[֐-׿]"),"devanagari":re.compile(r"[ऀ-ॿ]"),"thai":re.compile(r"[ก-฿]"),"cyrillic":re.compile(r"[Ѐ-ӿ]")}
def detect_scripts(text:str)->list[str]: return [name for name,pattern in SCRIPT_PATTERNS.items() if pattern.search(text)]
def safe_text(value:Any)->str: return "" if value is None else str(value)
def main(session:Session,p_source_table:str,p_text_column:str,p_limit:int)->dict[str,Any]:
 if not re.fullmatch(r"[A-Za-z0-9_.$\"]+",p_source_table): raise ValueError("Unsafe source table identifier")
 if not re.fullmatch(r"[A-Za-z0-9_$\"]+",p_text_column): raise ValueError("Unsafe text column identifier")
 rows=session.table(p_source_table).select(col(p_text_column).cast("string").alias("TEXT_VALUE")).limit(max(0,min(int(p_limit),10000))).collect();script_counts=Counter();category_counts=Counter();normalization_changes=0;examples=[]
 for row in rows:
  original=safe_text(row["TEXT_VALUE"]);normalized=unicodedata.normalize("NFKC",original);scripts=detect_scripts(original)
  if original!=normalized: normalization_changes+=1
  for script in scripts or ["latin_or_other"]: script_counts[script]+=1
  for character in original: category_counts[unicodedata.category(character)]+=1
  if len(examples)<12: examples.append({"original":original,"normalized":normalized,"scripts":scripts,"code_points":[f"U+{ord(ch):04X}" for ch in original[:24]]})
 return {"status":"OK","source_table":p_source_table,"text_column":p_text_column,"row_count":len(rows),"normalization_changes":normalization_changes,"script_counts":dict(sorted(script_counts.items())),"unicode_category_counts":dict(sorted(category_counts.items())),"examples":examples,"messages":{"ja":"Unicodeプロファイルが完了しました。","ko":"유니코드 프로파일링이 완료되었습니다.","ar":"اكتمل تحليل يونيكود.","hi":"यूनिकोड प्रोफ़ाइल पूरी हुई।"}}
$$;

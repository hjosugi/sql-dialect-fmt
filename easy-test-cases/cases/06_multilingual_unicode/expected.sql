-- Case 06: multilingual text, quoted identifiers, Unicode normalization traps,
-- right-to-left strings, emoji, combining marks, and embedded JSON.
CREATE OR REPLACE TEMPORARY TABLE CORE."多言語_顧客ラベル" (
    "顧客ID" VARCHAR NOT NULL,
    "表示名" VARCHAR,
    "日本語" VARCHAR,
    "한국어" VARCHAR,
    "简体中文" VARCHAR,
    "العربية" VARCHAR,
    "עברית" VARCHAR,
    "हिन्दी" VARCHAR,
    "ภาษาไทย" VARCHAR,
    "emoji_😀" VARCHAR,
    "metadata_メタデータ" VARIANT,
    created_at TIMESTAMP_TZ DEFAULT CURRENT_TIMESTAMP()
);

INSERT INTO CORE."多言語_顧客ラベル" (
    "顧客ID",
    "表示名",
    "日本語",
    "한국어",
    "简体中文",
    "العربية",
    "עברית",
    "हिन्दी",
    "ภาษาไทย",
    "emoji_😀",
    "metadata_メタデータ"
)
SELECT
    column1,
    column2,
    column3,
    column4,
    column5,
    column6,
    column7,
    column8,
    column9,
    column10,
    PARSE_JSON(column11)
FROM VALUES
    (
        'C-JP-001',
        '杉野尾 広貴',
        '東京都で「雪」と桜を見る。',
        '서울에서 데이터 파이프라인을 운영합니다.',
        '你好，世界；数据工程。',
        'مرحبًا بالعالم — هندسة البيانات',
        'שלום עולם — הנדסת נתונים',
        'नमस्ते दुनिया — डेटा इंजीनियरिंग',
        'สวัสดีชาวโลก — วิศวกรรมข้อมูล',
        '👩🏽‍💻🚀📦✅',
        '{"locale":"ja-JP","tags":["雪","데이터","بيانات"],"nested":{"fullwidth":"ＡＢＣ１２３","nfd":"Café"}}'
    ),
    (
        'C-EU-002',
        'Élodie d''Arcy',
        'L''été, le café et la crème brûlée.',
        '따옴표 '' 및 세미콜론 ; 테스트',
        '引号“测试”、分号；以及换行',
        'اختبار علامات الاقتباس '' والفاصلة المنقوطة ؛',
        'בדיקת ציטוטים '' ונקודה-פסיק ;',
        'उद्धरण '' और अर्धविराम ; परीक्षण',
        'ทดสอบอัญประกาศ '' และอัฒภาค ;',
        '🏳️‍🌈👨‍👩‍👧‍👦🧪',
        '{"locale":"fr-FR","currency":"EUR","rtl":false,"text":"line1\\nline2\\tend"}'
    );

SELECT
    "顧客ID" AS customer_id,
    "表示名" AS display_name,
    LENGTH("表示名") AS character_count,
    OCTET_LENGTH("表示名") AS utf8_byte_count,
    REGEXP_REPLACE("表示名", '[[:space:]]+', ' ') AS collapsed_spaces,
    "metadata_メタデータ":nested.fullwidth::STRING AS fullwidth_text,
    "metadata_メタデータ":nested.nfd::STRING AS potentially_decomposed_text,
    ARRAY_SIZE("metadata_メタデータ":tags) AS tag_count,
    OBJECT_KEYS("metadata_メタデータ") AS metadata_keys,
    CASE
        WHEN "العربية" IS NOT NULL THEN 'rtl-and-ltr-mixed'
        WHEN "עברית" IS NOT NULL THEN 'hebrew-present'
        ELSE 'other'
    END AS direction_test,
    CONCAT_WS(
        ' | ',
        NULLIF("日本語", ''),
        NULLIF("한국어", ''),
        NULLIF("简体中文", ''),
        NULLIF("العربية", ''),
        NULLIF("עברית", '')
    ) AS multilingual_summary
FROM CORE."多言語_顧客ラベル"
WHERE
    "日本語" COLLATE 'en-ci' ILIKE '%データ%'
    OR "한국어" ILIKE '%데이터%'
    OR "العربية" ILIKE '%بيانات%'
ORDER BY "表示名" COLLATE 'en-ci';

SELECT
    label."顧客ID",
    tag.index AS tag_index,
    tag.value::STRING AS tag_value,
    TYPEOF(tag.value) AS tag_type
FROM CORE."多言語_顧客ラベル" AS label,
    LATERAL FLATTEN(
        INPUT => label."metadata_メタデータ":tags,
        OUTER => TRUE
    ) AS tag
ORDER BY label."顧客ID", tag.index;

-- 02_seed.sql
INSERT INTO CORE.PRODUCTS (
    product_id,
    sku,
    product_name,
    category,
    unit_price,
    currency
)
SELECT
    column1,
    column2,
    PARSE_JSON(column3),
    column4,
    column5,
    column6
FROM VALUES
    ('P001', 'BOOK-JP-001', '{"ja":"分散システム入門","en":"Introduction to Distributed Systems"}', 'BOOKS', 3200.00, 'JPY'),
    ('P002', 'TEA-KYOTO-02', '{"ja":"宇治抹茶","en":"Uji Matcha","ko":"우지 말차"}', 'FOOD', 1800.00, 'JPY'),
    ('P003', 'CAFÉ-FR-03', '{"fr":"Café de spécialité","en":"Specialty Coffee"}', 'FOOD', 18.50, 'EUR'),
    ('P004', 'DATA-KR-04', '{"ko":"데이터 파이프라인 노트","en":"Data Pipeline Notebook"}', 'STATIONERY', 22000.00, 'KRW'),
    ('P005', 'MAP-AR-05', '{"ar":"خريطة السفر الذكية","en":"Smart Travel Map"}', 'TRAVEL', 75.00, 'SAR');

INSERT INTO CORE.MULTILINGUAL_TEXTS (
    text_id,
    language_tag,
    text_value,
    metadata
)
SELECT
    column1,
    column2,
    column3,
    PARSE_JSON(column4)
FROM VALUES
    ('T001', 'ja-JP', '東京都で雪と桜を見る。', '{"script":"Jpan","emoji":"🌸"}'),
    ('T002', 'ko-KR', '서울에서 데이터 파이프라인을 운영합니다.', '{"script":"Kore"}'),
    ('T003', 'zh-CN', '你好，世界；数据工程。', '{"script":"Hans"}'),
    ('T004', 'ar-SA', 'مرحبًا بالعالم — هندسة البيانات', '{"script":"Arab","direction":"rtl"}'),
    ('T005', 'he-IL', 'שלום עולם — הנדסת נתונים', '{"script":"Hebr","direction":"rtl"}'),
    ('T006', 'hi-IN', 'नमस्ते दुनिया — डेटा इंजीनियरिंग', '{"script":"Deva"}'),
    ('T007', 'th-TH', 'สวัสดีชาวโลก — วิศวกรรมข้อมูล', '{"script":"Thai"}'),
    ('T008', 'fr-FR', 'L''été, le café et la crème brûlée.', '{"script":"Latn","accented":true}'),
    ('T009', 'und', 'ＡＢＣ１２３ / Café / 👩🏽‍💻🚀', '{"normalization":"mixed","contains_nfd":true}');

INSERT INTO RAW.CUSTOMER_EVENTS (
    event_id,
    batch_id,
    event_time,
    event_type,
    source_system,
    payload
)
SELECT
    column1,
    column2,
    column3::TIMESTAMP_TZ,
    column4,
    column5,
    PARSE_JSON(column6)
FROM VALUES
    ('CE001', 'BATCH-001', '2026-06-01 09:00:00 +09:00', 'UPSERT', 'web-ja', '{"customer_id":"C001","display_name":"杉野尾 広貴","email":"HIROKI@example.jp ","locale":"ja-JP","region":"JP","marketing_opt_in":true,"attributes":{"interests":["cloud","分散システム"],"tier":"gold"}}'),
    ('CE002', 'BATCH-001', '2026-06-01 10:00:00 +02:00', 'UPSERT', 'web-fr', '{"customer_id":"C002","display_name":"Élodie d''Arcy","email":"elodie@example.fr","locale":"fr-FR","region":"FR","marketing_opt_in":false,"attributes":{"interests":["café","data"],"tier":"silver"}}'),
    ('CE003', 'BATCH-001', '2026-06-01 17:00:00 +09:00', 'UPSERT', 'mobile-ko', '{"customer_id":"C003","display_name":"김민수","email":"minsu@example.kr","locale":"ko-KR","region":"KR","marketing_opt_in":true,"attributes":{"interests":["데이터","AI"],"tier":"gold"}}'),
    ('CE004', 'BATCH-001', '2026-06-01 12:00:00 +03:00', 'UPSERT', 'mobile-ar', '{"customer_id":"C004","display_name":"ليان أحمد","email":"layan@example.sa","locale":"ar-SA","region":"SA","marketing_opt_in":true,"attributes":{"interests":["السفر","الذكاء الاصطناعي"],"tier":"bronze"}}'),
    ('CE005', 'BATCH-001', '2026-06-01 14:00:00 +05:30', 'UPSERT', 'web-hi', '{"customer_id":"C005","display_name":"आरव शर्मा","email":"aarav@example.in","locale":"hi-IN","region":"IN","marketing_opt_in":false,"attributes":{"interests":["क्लाउड","डेटा"],"tier":"silver"}}'),
    ('CE006', 'BATCH-001', '2026-06-01 13:00:00 +03:00', 'UPSERT', 'web-he', '{"customer_id":"C006","display_name":"נועה לוי","email":"noa@example.il","locale":"he-IL","region":"IL","marketing_opt_in":true,"attributes":{"interests":["ענן","נתונים"],"tier":"gold"}}');

INSERT INTO RAW.ORDER_EVENTS (
    event_id,
    batch_id,
    event_time,
    event_type,
    source_system,
    payload
)
SELECT
    column1,
    column2,
    column3::TIMESTAMP_TZ,
    column4,
    column5,
    PARSE_JSON(column6)
FROM VALUES
    ('OE001', 'BATCH-001', '2026-06-02 09:05:00 +09:00', 'UPSERT', 'checkout-ja', '{"order_id":"O1001","customer_id":"C001","status":"PAID","order_time":"2026-06-02T09:00:00+09:00","currency":"JPY","discount_amount":500,"shipping_address":{"country":"JP","city":"西東京市","line1":"下保谷3-17-14"},"items":[{"line":1,"product_id":"P001","quantity":1,"unit_price":3200},{"line":2,"product_id":"P002","quantity":2,"unit_price":1800}]}'),
    ('OE002', 'BATCH-001', '2026-06-02 10:10:00 +02:00', 'UPSERT', 'checkout-fr', '{"order_id":"O1002","customer_id":"C002","status":"PAID","order_time":"2026-06-02T10:00:00+02:00","currency":"EUR","discount_amount":0,"shipping_address":{"country":"FR","city":"Lyon"},"items":[{"line":1,"product_id":"P003","quantity":3,"unit_price":18.5}]}'),
    ('OE003', 'BATCH-001', '2026-06-02 18:30:00 +09:00', 'UPSERT', 'checkout-ko', '{"order_id":"O1003","customer_id":"C003","status":"SHIPPED","order_time":"2026-06-02T18:00:00+09:00","currency":"KRW","discount_amount":2000,"shipping_address":{"country":"KR","city":"서울"},"items":[{"line":1,"product_id":"P004","quantity":2,"unit_price":22000},{"line":2,"product_id":"P002","quantity":1,"unit_price":1800}]}'),
    ('OE004', 'BATCH-001', '2026-06-02 13:00:00 +03:00', 'UPSERT', 'checkout-ar', '{"order_id":"O1004","customer_id":"C004","status":"CREATED","order_time":"2026-06-02T12:55:00+03:00","currency":"SAR","discount_amount":5,"shipping_address":{"country":"SA","city":"الرياض"},"items":[{"line":1,"product_id":"P005","quantity":1,"unit_price":75}]}'),
    ('OE005', 'BATCH-001', '2026-06-02 15:00:00 +05:30', 'UPSERT', 'checkout-hi', '{"order_id":"O1005","customer_id":"C005","status":"DELIVERED","order_time":"2026-06-02T14:45:00+05:30","currency":"JPY","discount_amount":200,"shipping_address":{"country":"IN","city":"दिल्ली"},"items":[{"line":1,"product_id":"P001","quantity":1,"unit_price":3200},{"line":2,"product_id":"P004","quantity":1,"unit_price":22000}]}');

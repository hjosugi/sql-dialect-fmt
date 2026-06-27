-- CASE, CAST, :: cast, and || concatenation.
SELECT
    CASE WHEN amount > 100 THEN 'big' ELSE 'small' END AS bucket,
    CAST(amount AS decimal(10, 2)) AS amt,
    id::string AS id_str,
    region || '-' || product AS combo
FROM main.default.sales;

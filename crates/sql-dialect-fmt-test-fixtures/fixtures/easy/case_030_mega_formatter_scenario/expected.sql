-- Case 030: Mega scenario combining DDL, CTE, MERGE, task call, comments, Unicode, and deep nesting.
CREATE OR REPLACE TEMPORARY TABLE QA.MEGA_FORMATTER_INPUT (
    tenant_id STRING,
    document_id STRING,
    version NUMBER,
    payload VARIANT,
    created_at TIMESTAMP_TZ DEFAULT CURRENT_TIMESTAMP(),
    /* Block comment with SQL-looking text: SELECT * FROM fake WHERE x = 'not real'; */
    CONSTRAINT PK_MEGA_FORMATTER_INPUT PRIMARY KEY (tenant_id, document_id, version) NOT ENFORCED
)
COMMENT = 'Formatter stress table: 日本語, 한국어, العربية, emoji-like text inside strings only';

MERGE INTO QA.MEGA_FORMATTER_RESULT AS target
USING (
    WITH base AS (
        SELECT
            tenant_id,
            document_id,
            version,
            payload,
            created_at,
            payload:meta:source::STRING AS source_name,
            COALESCE(payload:meta:locale::STRING, 'und') AS locale,
            ROW_NUMBER() OVER (
                PARTITION BY tenant_id, document_id
                ORDER BY version DESC, created_at DESC
            ) AS version_rank
        FROM QA.MEGA_FORMATTER_INPUT
        WHERE payload IS NOT NULL
        QUALIFY version_rank = 1
    ), sections AS (
        SELECT
            b.*,
            s.index AS section_index,
            s.value:title::STRING AS section_title,
            s.value:body::STRING AS section_body,
            s.value:tokens AS section_tokens
        FROM base AS b,
            LATERAL FLATTEN(INPUT => b.payload:sections, OUTER => TRUE) AS s
    ), tokens AS (
        SELECT
            s.tenant_id,
            s.document_id,
            s.version,
            s.locale,
            s.source_name,
            s.section_index,
            s.section_title,
            t.index AS token_index,
            t.value:text::STRING AS token_text,
            t.value:kind::STRING AS token_kind,
            TRY_TO_DOUBLE(t.value:score::STRING) AS token_score
        FROM sections AS s,
            LATERAL FLATTEN(INPUT => s.section_tokens, OUTER => TRUE) AS t
    ), scored AS (
        SELECT
            tenant_id,
            document_id,
            version,
            locale,
            source_name,
            section_index,
            section_title,
            COUNT_IF(token_kind = 'keyword') AS keyword_count,
            COUNT_IF(token_kind = 'identifier') AS identifier_count,
            AVG(token_score) AS avg_token_score,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'text', token_text,
                    'kind', token_kind,
                    'score', token_score
                )
            ) WITHIN GROUP (ORDER BY token_index) AS tokens
        FROM tokens
        GROUP BY tenant_id, document_id, version, locale, source_name, section_index, section_title
    ), rolled AS (
        SELECT
            tenant_id,
            document_id,
            version,
            locale,
            source_name,
            ARRAY_AGG(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'index', section_index,
                    'title', section_title,
                    'keywords', keyword_count,
                    'identifiers', identifier_count,
                    'avg_score', avg_token_score,
                    'tokens', tokens
                )
            ) WITHIN GROUP (ORDER BY section_index) AS section_summary,
            SUM(keyword_count) AS keyword_count,
            SUM(identifier_count) AS identifier_count,
            AVG(avg_token_score) AS document_score
        FROM scored
        GROUP BY tenant_id, document_id, version, locale, source_name
    )
    SELECT
        tenant_id,
        document_id,
        version,
        locale,
        source_name,
        section_summary,
        keyword_count,
        identifier_count,
        document_score,
        SHA2_HEX(
            TO_JSON(
                OBJECT_CONSTRUCT_KEEP_NULL(
                    'version', version,
                    'locale', locale,
                    'source', source_name,
                    'sections', section_summary,
                    'keyword_count', keyword_count,
                    'identifier_count', identifier_count,
                    'document_score', document_score
                )
            ),
            256
        ) AS result_hash
    FROM rolled
) AS source
    ON target.tenant_id = source.tenant_id
    AND target.document_id = source.document_id
WHEN MATCHED AND target.result_hash IS DISTINCT FROM source.result_hash THEN
    UPDATE SET
        target.version = source.version,
        target.locale = source.locale,
        target.source_name = source.source_name,
        target.section_summary = source.section_summary,
        target.keyword_count = source.keyword_count,
        target.identifier_count = source.identifier_count,
        target.document_score = source.document_score,
        target.result_hash = source.result_hash,
        target.updated_at = CURRENT_TIMESTAMP()
WHEN NOT MATCHED THEN
    INSERT (
        tenant_id,
        document_id,
        version,
        locale,
        source_name,
        section_summary,
        keyword_count,
        identifier_count,
        document_score,
        result_hash,
        created_at,
        updated_at
    )
    VALUES (
        source.tenant_id,
        source.document_id,
        source.version,
        source.locale,
        source.source_name,
        source.section_summary,
        source.keyword_count,
        source.identifier_count,
        source.document_score,
        source.result_hash,
        CURRENT_TIMESTAMP(),
        CURRENT_TIMESTAMP()
    );

CALL OPS.SP_RECORD_FORMATTER_CASE_RESULT(
    'case_030',
    OBJECT_CONSTRUCT(
        'note', 'Nested formatter stress case completed / 多重ネスト完了',
        'query_id', LAST_QUERY_ID(),
        'checked_at', CURRENT_TIMESTAMP()
    )
);

-- Case 002: Recursive hierarchy rollup with cycle guard and grouped analytics.
WITH RECURSIVE org_tree AS (
    SELECT
        e.employee_id,
        e.manager_id,
        e.department_id,
        e.region,
        e.hire_date,
        e.employee_name,
        0 AS depth,
        ARRAY_CONSTRUCT(e.employee_id) AS path_ids,
        TO_VARCHAR(e.employee_id) AS path_text
    FROM HR.EMPLOYEES AS e
    WHERE e.manager_id IS NULL

    UNION ALL

    SELECT
        child.employee_id,
        child.manager_id,
        child.department_id,
        child.region,
        child.hire_date,
        child.employee_name,
        parent.depth + 1 AS depth,
        ARRAY_APPEND(parent.path_ids, child.employee_id) AS path_ids,
        parent.path_text || ' > ' || TO_VARCHAR(child.employee_id) AS path_text
    FROM HR.EMPLOYEES AS child
        INNER JOIN org_tree AS parent
            ON child.manager_id = parent.employee_id
    WHERE
        parent.depth < 30
        AND NOT ARRAY_CONTAINS(child.employee_id::VARIANT, parent.path_ids)
), enriched AS (
    SELECT
        t.*,
        d.department_name,
        DATEDIFF('day', t.hire_date, CURRENT_DATE()) AS tenure_days,
        COUNT(*) OVER (
            PARTITION BY t.manager_id
        ) AS sibling_count,
        DENSE_RANK() OVER (
            PARTITION BY t.department_id
            ORDER BY t.depth DESC, t.hire_date ASC
        ) AS depth_rank_in_department
    FROM org_tree AS t
        LEFT JOIN HR.DEPARTMENTS AS d
            ON t.department_id = d.department_id
), rollup_rows AS (
    SELECT
        region,
        department_name,
        depth,
        GROUPING(region) AS g_region,
        GROUPING(department_name) AS g_department,
        GROUPING(depth) AS g_depth,
        COUNT(*) AS employee_count,
        APPROX_COUNT_DISTINCT(manager_id) AS manager_count,
        MAX(depth) AS max_depth,
        AVG(tenure_days) AS avg_tenure_days,
        ARRAY_AGG(employee_name) WITHIN GROUP (ORDER BY depth DESC, hire_date ASC) AS sample_names
    FROM enriched
    GROUP BY ROLLUP(region, department_name, depth)
)
SELECT
    CASE WHEN g_region = 1 THEN '<ALL_REGIONS>' ELSE region END AS region_label,
    CASE WHEN g_department = 1 THEN '<ALL_DEPARTMENTS>' ELSE department_name END AS department_label,
    CASE WHEN g_depth = 1 THEN -1 ELSE depth END AS depth_bucket,
    employee_count,
    manager_count,
    max_depth,
    ROUND(avg_tenure_days, 2) AS avg_tenure_days,
    sample_names[0]::STRING AS first_sample_employee,
    RATIO_TO_REPORT(employee_count) OVER (
        PARTITION BY g_region, region
    ) AS share_in_region
FROM rollup_rows
WHERE employee_count >= 1
ORDER BY g_region, region_label, g_department, department_label, depth_bucket;

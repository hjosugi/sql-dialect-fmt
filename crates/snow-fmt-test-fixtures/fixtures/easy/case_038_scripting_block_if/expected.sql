DECLARE
    threshold int default 100;
    result string;
BEGIN
    LET total := (SELECT sum(amount) FROM orders);
    IF (total > threshold) THEN
        result := 'high';
    ELSEIF (total > 0) THEN
        result := 'low';
    ELSE
        result := 'none';
    END IF;
    RETURN result;
END;

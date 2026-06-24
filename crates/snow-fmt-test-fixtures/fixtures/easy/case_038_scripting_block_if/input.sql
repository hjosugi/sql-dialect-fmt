declare
  threshold int default 100;
  result string;
begin
  let total := (select sum(amount) from orders);
  if (total > threshold) then
    result := 'high';
  elseif (total > 0) then
    result := 'low';
  else
    result := 'none';
  end if;
  return result;
end;

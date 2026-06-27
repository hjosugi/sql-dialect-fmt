begin
  for rec in (select id from tasks) do
    insert into processed (id) values (rec.id);
  end for;
exception
  when statement_error then
    rollback;
    return 'failed';
  when other then
    return 'unknown';
end;

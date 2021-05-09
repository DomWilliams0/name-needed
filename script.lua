-- very temporary script for dev

local e = GetEntityById("E1:3");
SelectEntity(e);
--UnselectEntity();
info("lmao i am a script");

for k,v in pairs(_G) do debug(string.format("%s => %s", k, v)) end

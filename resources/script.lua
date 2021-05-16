-- very hacky script for messing around with lua api

local e = GetEntityById("E1:3");
SelectEntity(e);
-- UnselectEntity();
print("selected an entity for you!")

local society = GetPlayerSociety()
print(string.format("player society = %s", society))

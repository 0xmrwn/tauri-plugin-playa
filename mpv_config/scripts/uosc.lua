-- Load uosc main script
local mp = require('mp')
local msg = mp.msg
local utils = require('mp.utils')

-- Get the config directory
local config_dir = mp.get_property("config-dir")
msg.info("Config directory: " .. (config_dir or "nil"))

-- Define paths
local scripts_dir = utils.join_path(config_dir, "scripts")
local uosc_dir = utils.join_path(scripts_dir, "uosc")
local main_script = utils.join_path(uosc_dir, "main.lua")

-- Add uosc directory to Lua package path
msg.info("Setting package path to include: " .. uosc_dir)
package.path = package.path .. ";" .. uosc_dir .. "/?.lua;" .. uosc_dir .. "/?/init.lua"

-- Load the main uosc script
msg.info("Loading uosc from: " .. main_script)
local file_info = utils.file_info(main_script)
if file_info then
    dofile(main_script)
else
    msg.error("Could not find uosc main script at: " .. main_script)
end 
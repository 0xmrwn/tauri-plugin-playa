require('lib/text')

-- Add logging
local mp = require('mp')
local msg = mp.msg
local utils = require('mp.utils')

msg.info("char_conv.lua: Starting to load")

-- Get script directory with fallback
local script_dir = mp.get_script_directory()
msg.info("char_conv.lua: script_dir = " .. (script_dir or "nil"))

if not script_dir then
	local config_dir = mp.get_property("config-dir")
	msg.info("char_conv.lua: config_dir = " .. (config_dir or "nil"))
	script_dir = utils.join_path(config_dir, "scripts/uosc")
	msg.info("char_conv.lua: Using fallback script_dir = " .. script_dir)
end

local char_dir = script_dir .. '/char-conv/'
msg.info("char_conv.lua: char_dir = " .. char_dir)

local data = {}

-- Check if get_languages exists
if type(get_languages) ~= "function" then
	msg.error("char_conv.lua: get_languages function is not defined!")
	-- Define a fallback
	get_languages = function() return {"en"} end
end

local languages = get_languages()
msg.info("char_conv.lua: languages = " .. utils.format_json(languages))

for _, lang in ipairs(languages) do
	-- Check if get_locale_from_json exists
	if type(get_locale_from_json) ~= "function" then
		msg.error("char_conv.lua: get_locale_from_json function is not defined!")
		break
	end
	
	local json_path = char_dir .. lang:lower() .. '.json'
	msg.info("char_conv.lua: Loading " .. json_path)
	local locale_data = get_locale_from_json(json_path)
	if locale_data then
		table_assign(data, locale_data)
	else
		msg.warn("char_conv.lua: Failed to load " .. json_path)
	end
end

local romanization = {}

local function get_romanization_table()
	for k, v in pairs(data) do
		for _, char in utf8_iter(v) do
			romanization[char] = k
		end
	end
end
get_romanization_table()

function need_romanization()
	return next(romanization) ~= nil
end

function char_conv(chars, use_ligature, has_separator)
	local separator = has_separator or ' '
	local length = 0
	local char_conv, sp, cache = {}, {}, {}
	local chars_length = utf8_length(chars)
	local concat = table.concat
	for _, char in utf8_iter(chars) do
		if use_ligature then
			if #char == 1 then
				char_conv[#char_conv + 1] = char
			else
				char_conv[#char_conv + 1] = romanization[char] or char
			end
		else
			length = length + 1
			if #char <= 2 then
				if (char ~= ' ' and length ~= chars_length) then
					cache[#cache + 1] = romanization[char] or char
				elseif (char == ' ' or length == chars_length) then
					if length == chars_length then
						cache[#cache + 1] = romanization[char] or char
					end
					sp[#sp + 1] = concat(cache)
					itable_clear(cache)
				end
			else
				if next(cache) ~= nil then
					sp[#sp + 1] = concat(cache)
					itable_clear(cache)
				end
				sp[#sp + 1] = romanization[char] or char
			end
		end
	end
	if use_ligature then
		return concat(char_conv)
	else
		return concat(sp, separator)
	end
end

return char_conv

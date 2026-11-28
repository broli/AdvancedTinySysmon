local function readStatus()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		return nil
	end
	local data = noctalia.json.decode(raw)
	return data
end

-- Resolves dotted paths even when device IDs contain dots (e.g. gpu.pci-0000:01:00.0.temp_c).
local function resolvePath(node, path)
	if type(node) ~= "table" or type(path) ~= "string" then
		return nil
	end
	if node[path] ~= nil then
		return node[path]
	end
	for key, val in pairs(node) do
		local prefix = key .. "."
		if string.sub(path, 1, #prefix) == prefix then
			return resolvePath(val, string.sub(path, #prefix + 1))
		end
	end
	return nil
end

local function formatValue(val)
	if type(val) == "number" then
		if val == math.floor(val) then
			return tostring(val)
		end
		return string.format("%.1f", val)
	end
	return tostring(val)
end

local function formatHumanSize(val, path)
	if type(val) ~= "number" then
		return tostring(val)
	end
	local bytes = val
	if string.sub(path, -3) == "_kb" then
		bytes = val * 1024
	elseif string.sub(path, -3) == "_mb" then
		bytes = val * 1024 * 1024
	end
	local units = { "B", "K", "M", "G", "T", "P" }
	local idx = 1
	while bytes >= 1024 and idx < #units do
		bytes = bytes / 1024
		idx = idx + 1
	end
	if idx == 1 then
		return string.format("%d%s", math.floor(bytes + 0.5), units[idx])
	end
	return string.format("%.1f%s", bytes, units[idx])
end

local function applyFormat(label, val, path)
	if string.find(label, "{", 1, true) then
		local out = string.gsub(label, "{([^}]*)}", function(spec)
			if spec == "" then
				return formatValue(val)
			elseif spec == "h" or spec == "human" then
				return formatHumanSize(val, path)
			elseif type(val) == "number" then
				local w, d = string.match(spec, "^0(%d+)%.(%d+)$")
				if w and d then
					return string.format("%0" .. w .. "." .. d .. "f", val)
				end
				local width = string.match(spec, "^0(%d+)$")
				if width then
					return string.format("%0" .. width .. "d", math.floor(val + 0.5))
				end
			end
			return formatValue(val)
		end)
		return out
	elseif label == "" then
		return formatValue(val)
	else
		return label .. " " .. formatValue(val)
	end
end

noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)

function update()
	local metrics = noctalia.getConfig("metrics")
	if type(metrics) ~= "table" or next(metrics) == nil then
		barWidget.setText("adsymon: add metrics")
		return
	end

	local data = readStatus()
	if not data then
		barWidget.setText("adsymon: offline")
		return
	end

	local labels = {}
	for label in pairs(metrics) do
		table.insert(labels, label)
	end
	table.sort(labels)

	local parts = {}
	for _, key in ipairs(labels) do
		local rawVal = metrics[key]
		local val = resolvePath(data, key)
		local path, fmt
		if val ~= nil then
			path = key
			fmt = rawVal
		else
			val = resolvePath(data, rawVal)
			path = rawVal
			fmt = key
		end
		if val ~= nil then
			table.insert(parts, applyFormat(fmt, val, path))
		end
	end

	local sep = noctalia.getConfig("separator") or "  |  "
	if #parts == 0 then
		barWidget.setText("adsymon: --")
	else
		barWidget.setText(table.concat(parts, sep))
	end
end

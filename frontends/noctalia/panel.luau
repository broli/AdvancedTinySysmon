local function readStatus()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		return nil, path
	end
	return noctalia.json.decode(raw), path
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
		return (string.gsub(label, "{([^}]*)}", function(spec)
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
		end))
	elseif label == "" then
		return formatValue(val)
	else
		return label .. " " .. formatValue(val)
	end
end

local function suggestFormat(path)
	if string.sub(path, -4) == "_pct" then
		return "{03}%"
	elseif string.sub(path, -2) == "_c" then
		return "{02}°C"
	elseif string.sub(path, -4) == "_mhz" then
		return "{04} MHz"
	elseif string.sub(path, -4) == ".rpm" then
		return "{04} RPM"
	elseif string.sub(path, -3) == "_kb" or string.sub(path, -3) == "_mb" then
		return "{h}"
	elseif string.sub(path, -3) == "_ms" then
		return "{} ms"
	end
	return "{}"
end

local function collectMetrics(node, prefix, category, out)
	if type(node) ~= "table" then
		return
	end
	for key, val in pairs(node) do
		if key ~= "timestamp_ms" then
			local path = prefix == "" and key or (prefix .. "." .. key)
			local cat = category ~= "" and category or key
			if type(val) == "table" then
				collectMetrics(val, path, cat, out)
			else
				table.insert(out, {
					category = cat,
					path = path,
					value = val,
				})
			end
		end
	end
end

local selectedCategoryIdx = 1
local selectedMetricIdx = 1
local searchQuery = ""
local formatText = "{}%"
local lastPickedPath = nil
local copiedBanner = nil

local function render()
	local data, statusPath = readStatus()
	if not data then
		panel.render(ui.column({ flexGrow = 1, gap = 12, padding = 12 }, {
			ui.row({ align = "center", justify = "space_between" }, {
				ui.label({ text = "Adsymon Quick Setup", fontSize = 15, fontWeight = "bold", color = "on_surface" }),
				ui.button({ glyph = "close", variant = "ghost", onClick = function() panel.close() end }),
			}),
			ui.label({
				text = "adsymon is offline (could not read " .. statusPath .. ")",
				color = "error",
			}),
		}))
		return
	end

	local allMetrics = {}
	collectMetrics(data, "", "", allMetrics)
	table.sort(allMetrics, function(a, b)
		return a.path < b.path
	end)

	local catSet = {}
	local categories = { "All" }
	for _, m in ipairs(allMetrics) do
		if not catSet[m.category] then
			catSet[m.category] = true
			table.insert(categories, m.category)
		end
	end
	table.sort(categories, function(a, b)
		if a == "All" then return true end
		if b == "All" then return false end
		return a < b
	end)
	if selectedCategoryIdx > #categories then
		selectedCategoryIdx = 1
	end
	local activeCat = categories[selectedCategoryIdx]

	local filtered = {}
	local q = string.lower(searchQuery)
	for _, m in ipairs(allMetrics) do
		local catOk = (activeCat == "All") or (m.category == activeCat)
		local searchOk = (q == "") or (string.find(string.lower(m.path), q, 1, true) ~= nil)
		if catOk and searchOk then
			table.insert(filtered, m)
		end
	end

	if selectedMetricIdx > #filtered then
		selectedMetricIdx = 1
	end

	local metricOptions = {}
	for _, m in ipairs(filtered) do
		table.insert(metricOptions, m.path .. "  (" .. formatValue(m.value) .. ")")
	end
	if #metricOptions == 0 then
		table.insert(metricOptions, "(no matching metrics)")
	end

	local currentMetric = filtered[selectedMetricIdx]
	if currentMetric and currentMetric.path ~= lastPickedPath then
		lastPickedPath = currentMetric.path
		formatText = suggestFormat(currentMetric.path)
	end

	local previewText = "--"
	if currentMetric then
		previewText = applyFormat(formatText, currentMetric.value, currentMetric.path)
	end

	local rows = {}
	for _, m in ipairs(filtered) do
		local mPath = m.path
		table.insert(rows, ui.row({ align = "center", justify = "space_between", gap = 8 }, {
			ui.label({ text = mPath, fontSize = 12, color = "on_surface", flexGrow = 1 }),
			ui.label({ text = formatValue(m.value), fontSize = 12, fontWeight = "bold", color = "primary" }),
			ui.button({
				glyph = "copy",
				variant = "ghost",
				onClick = function()
					noctalia.copyToClipboard(mPath)
					copiedBanner = "Copied: " .. mPath
					render()
				end,
			}),
		}))
	end

	local children = {
		ui.row({ align = "center", justify = "space_between", gap = 8 }, {
			ui.row({ align = "center", gap = 6 }, {
				ui.glyph({ name = "cpu", color = "primary", size = 18 }),
				ui.label({ text = "Adsymon Quick Setup", fontSize = 15, fontWeight = "bold", color = "on_surface" }),
			}),
			ui.button({ glyph = "close", variant = "ghost", onClick = function() panel.close() end }),
		}),

		ui.row({ gap = 8, align = "center" }, {
			ui.select({
				key = "cat",
				options = categories,
				selectedIndex = selectedCategoryIdx - 1,
				onChange = function(idx)
					selectedCategoryIdx = (tonumber(idx) or 0) + 1
					selectedMetricIdx = 1
					render()
				end,
			}),
			ui.input({
				key = "search",
				value = searchQuery,
				placeholder = "Filter metrics...",
				flexGrow = 1,
				onChange = function(val)
					searchQuery = val or ""
					selectedMetricIdx = 1
					render()
				end,
			}),
		}),

		ui.column({ gap = 6 }, {
			ui.label({ text = "METRIC PICKER (FROM STATUS.JSON)", fontSize = 11, fontWeight = "bold", color = "on_surface_variant" }),
			ui.select({
				key = "metric",
				options = metricOptions,
				selectedIndex = selectedMetricIdx - 1,
				onChange = function(idx)
					selectedMetricIdx = (tonumber(idx) or 0) + 1
					local picked = filtered[selectedMetricIdx]
					if picked then
						lastPickedPath = picked.path
						formatText = suggestFormat(picked.path)
					end
					render()
				end,
			}),
			ui.row({ gap = 8, align = "center" }, {
				ui.input({
					key = "format",
					value = formatText,
					placeholder = "Format (e.g. {}%)",
					flexGrow = 1,
					onChange = function(val)
						formatText = val or ""
						render()
					end,
				}),
				ui.label({ text = "Preview: " .. previewText, fontSize = 12, fontWeight = "bold", color = "primary" }),
			}),
			ui.row({ gap = 8 }, {
				ui.button({
					text = "Copy Format",
					glyph = "copy",
					variant = "secondary",
					flexGrow = 1,
					onClick = function()
						noctalia.copyToClipboard(formatText)
						copiedBanner = "Copied format: " .. formatText
						render()
					end,
				}),
				ui.button({
					text = "Copy Path",
					glyph = "copy",
					variant = "primary",
					flexGrow = 1,
					onClick = function()
						if currentMetric then
							noctalia.copyToClipboard(currentMetric.path)
							copiedBanner = "Copied path: " .. currentMetric.path
							render()
						end
					end,
				}),
			}),
		}),
	}

	if copiedBanner then
		table.insert(children, ui.label({ text = copiedBanner, fontSize = 11, color = "primary" }))
	end

	table.insert(children, ui.label({ text = "ALL LIVE METRICS (" .. tostring(#filtered) .. ")", fontSize = 11, fontWeight = "bold", color = "on_surface_variant" }))
	table.insert(children, ui.scroll({ key = "metrics-list", flexGrow = 1, gap = 4 }, rows))

	panel.render(ui.column({ flexGrow = 1, gap = 10, padding = 12 }, children))
end

noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)

function update()
	render()
end

render()

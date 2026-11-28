noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)
barWidget.setGlyph("cpu")

function update()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		barWidget.setText("--")
		return
	end

	local data = noctalia.json.decode(raw)
	local cpu = data and data.cpu
	if not cpu then
		barWidget.setText("--")
		return
	end

	local text = string.format("%.0f%%", cpu.usage_pct or 0)
	if noctalia.getConfig("show_temp") and cpu.temp_c then
		text = text .. string.format(" %.0f°C", cpu.temp_c)
	end
	barWidget.setText(text)
end

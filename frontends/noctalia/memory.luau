noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)
barWidget.setGlyph("memory")

function update()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		barWidget.setText("--")
		return
	end

	local data = noctalia.json.decode(raw)
	local mem = data and data.memory
	if not mem then
		barWidget.setText("--")
		return
	end

	local used_gb = (mem.used_kb or 0) / (1024 * 1024)
	local text = string.format("%.1fG (%.0f%%)", used_gb, mem.usage_pct or 0)
	if noctalia.getConfig("show_swap") and mem.swap_used_kb then
		local swap_gb = mem.swap_used_kb / (1024 * 1024)
		text = text .. string.format(" S:%.1fG", swap_gb)
	end
	barWidget.setText(text)
end

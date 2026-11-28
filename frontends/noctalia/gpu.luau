noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)
barWidget.setGlyph("gpu")

function update()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		barWidget.setText("--")
		return
	end

	local data = noctalia.json.decode(raw)
	local dev = noctalia.getConfig("device") or "pci-0000:01:00.0"
	local gpu = data and data.gpu and data.gpu[dev]
	if not gpu then
		barWidget.setText("--")
		return
	end

	local parts = {}
	if gpu.usage_pct ~= nil then
		table.insert(parts, string.format("%d%%", gpu.usage_pct))
	elseif gpu.freq_mhz ~= nil then
		table.insert(parts, string.format("%dMHz", gpu.freq_mhz))
	end

	if noctalia.getConfig("show_vram") and gpu.vram_used_mb ~= nil and gpu.vram_total_mb ~= nil then
		table.insert(parts, string.format("%.1f/%.1fG", gpu.vram_used_mb / 1024, gpu.vram_total_mb / 1024))
	end

	if noctalia.getConfig("show_temp") and gpu.temp_c ~= nil then
		table.insert(parts, string.format("%.0f°C", gpu.temp_c))
	end

	barWidget.setText(#parts > 0 and table.concat(parts, " ") or "--")
end

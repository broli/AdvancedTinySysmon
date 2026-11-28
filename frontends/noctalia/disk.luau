noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)
barWidget.setGlyph("hard-drive")

function update()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		barWidget.setText("--")
		return
	end

	local data = noctalia.json.decode(raw)
	local dev = noctalia.getConfig("device") or "nvme0n1"
	local disk = data and data.disk and data.disk[dev]
	if not disk then
		barWidget.setText("--")
		return
	end

	local text = string.format("%s", dev)
	if disk.temp_c ~= nil then
		text = text .. string.format(" %.0f°C", disk.temp_c)
	elseif disk.io_ticks_ms ~= nil then
		text = text .. string.format(" %dms", disk.io_ticks_ms)
	end
	barWidget.setText(text)
end

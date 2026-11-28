noctalia.setUpdateInterval(noctalia.getConfig("poll_interval_ms") or 1000)
barWidget.setGlyph("fan")

function update()
	local path = noctalia.getConfig("status_file") or "/run/user/1000/adsymon/status.json"
	local raw = noctalia.readFile(path)
	if not raw then
		barWidget.setText("--")
		return
	end

	local data = noctalia.json.decode(raw)
	local dev = noctalia.getConfig("device") or "hwmon4/fan1"
	local fan = data and data.fans and data.fans[dev]
	if not fan or fan.rpm == nil then
		barWidget.setText("--")
		return
	end

	barWidget.setText(string.format("%d RPM", fan.rpm))
end

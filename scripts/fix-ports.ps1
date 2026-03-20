Write-Host "Restarting WinNAT service..."
sudo net stop winnat
sudo net start winnat

# Write-Host "Restarting IP Helper service..."
# net stop iphlpsvc
# net start iphlpsvc

Write-Host "Done. Ports should be free now."
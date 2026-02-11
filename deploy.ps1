$src = "target\x86_64-myos\debug\bootimage-systemoperacyjny.bin"
$dst = "target\x86_64-myos\debug\bootimage-padded.bin"
$vdi = "C:\Users\fabia\Desktop\code\Systemoperacyjny\systemoperacyjny.vdi"
$vm_name = "Systemoperacyjny"
$vbox_manage = "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"

# Step 1: Detach medium from VM
Write-Host "Detaching old medium from VM..."
& $vbox_manage storageattach $vm_name --storagectl "IDE" --port 0 --device 0 --medium none 2>$null

# Step 2: Close/unregister old medium from VirtualBox registry
Write-Host "Closing old medium..."
& $vbox_manage closemedium disk $vdi --delete 2>$null

# Step 3: Pad binary to 1 MiB boundary
Write-Host "Padding binary..."
Copy-Item $src $dst -Force
$len = (Get-Item $dst).Length
$target = [math]::Ceiling($len / 1048576) * 1048576
if ($target -lt 1048576) { $target = 1048576 }
$missing = $target - $len
if ($missing -gt 0) {
    $fs = [System.IO.File]::OpenWrite($dst)
    $fs.Seek(0, [System.IO.SeekOrigin]::End) | Out-Null
    $zeros = New-Object byte[] $missing
    $fs.Write($zeros, 0, $zeros.Length)
    $fs.Close()
}
Write-Host "Padded to $target bytes"

# Step 4: Convert raw to VDI
Write-Host "Converting to VDI..."
if (Test-Path $vdi) { Remove-Item $vdi -Force }
& $vbox_manage convertfromraw $dst $vdi --format VDI

if ($LASTEXITCODE -ne 0) {
    Write-Host "Error converting to VDI"
    exit 1
}

# Step 5: Attach new VDI to VM
Write-Host "Attaching VDI to VM..."
& $vbox_manage storageattach $vm_name --storagectl "IDE" --port 0 --device 0 --type hdd --medium $vdi

if ($LASTEXITCODE -eq 0) {
    Write-Host "Success! VDI attached to VM '$vm_name'"
} else {
    Write-Host "Error attaching VDI to VM"
    exit 1
}

# Cleanup
Remove-Item $dst -Force -ErrorAction SilentlyContinue

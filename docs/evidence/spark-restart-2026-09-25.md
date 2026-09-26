# Spark unexpected restart investigation

The owner confirmed that they did not request this restart. The Avesra work
issued no reboot, firmware command, `mstflint` invocation or kernel change.
Read-only evidence is retained locally in ignored `artifacts/restart-20260925/`.

## Observed sequence (America/Chicago)

- 23:42:44: kernel `7.0.0-1019-nvidia` recorded an Oops in
  `pci_bus_read_config_dword`, reached through the `mstflint_access` ioctl path.
  The faulting process was root-owned `mstflint`, PID2531424.
- 23:43:55: RCU CPU stalls were recorded, followed by mlx5 timeouts. SSH stopped
  returning its banner while the host still answered ICMP at one observation.
- 23:44:55: last retained previous-boot journal entry.
- 23:46:37: the next boot began. Journald reported an unclean/corrupt previous
  journal and replaced it. Avesra's containers were stopped with exit255;
  the inspected reasoning container reported `OOMKilled:false`.

The bounded journal window contained no graceful shutdown/reboot request or OOM
event. These observations identify a kernel failure preceding the restart;
they do not establish who invoked the faulting command or what reset the host.
The post-boot `panic` and `panic_on_oops` settings were zero, which is not evidence
of their values at the earlier failure.

## Attribution boundary

Installed `/opt/nvidia/spark-ota-check/check_firmware.py` invokes a read-only
`mstflint -d <PCI-address> q` query. The installed firmware manager separately
contains a firmware-burn invocation. Neither source reference proves that it
launched PID2531424. The firmware-manager service had no entries around the
failure; its earlier previous-boot startup exited with a cable-related message.
No matching PID journal attribution was found. Do not describe this as a proven
firmware update, an Avesra memory failure, or a known automatic reboot.

The owner later reported plugging in a DAC cable about30minutes before23:53,
approximately23:23. A bounded23:15–23:40 log scan found Docker virtual-link
changes but no corresponding recorded physical carrier/hotplug event. ConnectX
DHCP attempts failed/retried at23:40:55 and23:41:40, before the23:42:44 Oops.
Installed `90-mtk-hotplug` rules invoke a handler that can enumerate/remove PCIe
devices; its inspected source does not invoke `mstflint`. This is relevant
hardware context, not proof of a cable-triggered crash. The cable was not
unplugged/replugged to reproduce it.

No firmware utility was rerun to reproduce the kernel fault. No package/kernel
upgrade, network reset, watchdog change or host reboot was performed. Future
investigation should preserve kernel and caller attribution before reproducing
any PCI/firmware query. A support report can use these logs without uploading
private model credentials, application databases or owner recordings.

## Avesra recovery implications

The transient controller did not survive the reboot, and the existing audio
containers have restart policy `no`. The separately owned reasoning controller's
user service existed but was not enabled. These are concrete deployment recovery
gaps; model load, model identity inspection and voice qualification remain
separate from service startup. Restore only Avesra-owned components with existing
private identity/configuration, preserve uncertain jobs, and do not restart the
unrelated Local Studio workload as a side effect.

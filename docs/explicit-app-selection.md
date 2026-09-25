# Explicit executable selection

Choose executable in visible Settings is implemented in source, with native picker, file fingerprint, explicit cwd and protected publication. It remains uninvoked and unqualified. It supplements metadata discovery; it does not accept a frontend/model path or launch the selected file.

The owned native setup coordinator opens IFileOpenDialog against the Settings HWND with filesystem/file-exists/path-exists/no-link-dereference/no-recent-entry options and an executable filter. Final native validation rejects nonlocal paths, non-executables, oversized files and a reparse point on the opened final file component (OPEN_REPARSE_POINT plus handle attributes). Ancestor junctions are not asserted absent; the canonical handle path and file identity are frozen and compared. a picker filter is not authority. The callback checks exact panel/connection/Windows-unlocked lifetime immediately before Show. The actual modal operation retains the work slot until it returns, even after IPC cancellation or Settings invalidation; only a current panel can publish its result.

The selected file receives a bounded native fingerprint (canonical path, SHA-256, size and volume/file ID), a native-generated candidate ID, and the existing two-minute candidate lifetime. The frontend receives display metadata, never the fingerprint as executable authority. No working directory is invented: an explicit native folder selection is required. The initial explicit-file flow supports empty arguments only and displays that fact. Start menu entries retain their separately discovered native arguments.

Fresh selection reopens/hashes the same file and rejects any identity drift before constructing a new immutable app UUID/revision under the actual owner. Window observation uses the same fingerprint and native hint flow as discovered executables. Final Remember rechecks the frozen fingerprint and optional window hint before the original one-use management proof/panel check. The catalog worker publishes the app and alias transactionally, without issuing a grant or accepting a task. No duplicate filename or display label can replace an existing UUID.

The UI uses the existing inline app chooser and approved Learned names geometry/tokens. A cancelled picker leaves existing choices intact; a successful selection stays within the aggregate candidate cap and identifies its explicit source. Close/lock/disconnect and expiry suppress stale publication. Modal cancellation is not falsely reported as stopping a Windows API that has not returned. No picker, file discovery or application effect will be invoked during the gaming restriction.

| Entry point | Responsibility |
| --- | --- |
| Native `choose_app_executable` command | Visible Settings and existing panel; owned coordinator through actual picker/fingerprint completion; no caller-supplied path |
| Native executable picker/fingerprint | Filesystem-only `.exe` selection; reject reparse/unsupported identity; bounded metadata and hash |
| Existing folder/window selection | Explicit cwd and native hint, using the same current candidate identity |
| Existing protected `remember_app` | Revalidate fingerprint and optional native hint before original proof/panel authorization on the catalog worker |
| Existing effect dispatch | Immutable target revalidation remains mandatory; selection does not grant dispatch |

Microsoft's [SetOptions contract](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-setoptions) governs dialog options; [SetFileTypes](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-setfiletypes) is applied once before Show and is not used on the separate folder picker. Static compilation and source review will verify this slice under the no-tests instruction; actual picker behavior remains unqualified.

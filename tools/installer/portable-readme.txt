Orchester for Windows (x64), portable

Start orchester.exe. Keep both programs in one folder: the app starts orchester-runner.exe from beside itself, and
without it the app cannot open a project.

The app keeps its data beside itself, in this folder: the application database orchester.db (with orchester.db-wal
and orchester.db-shm), orchester.lock and the logs folder. Choose a folder you can write to (not under Program
Files) and keep it. To update, close Orchester and replace only the two programs; the data stays. Before the first
start of a newer version, copy orchester.db (and its -wal file) aside: a newer version migrates it, and an older one
then refuses it.

The programs are not signed: Windows SmartScreen may warn when you start one (More info, then Run anyway).

The installer (Orchester-<version>-setup.exe on the same release page) installs the same programs for your user,
with a Start menu shortcut and an uninstaller.

param(
  [string]$Executable = 'E:\Dev\Avesra\target\release\avesra-desktop.exe',
  [Parameter(Mandatory=$true)][string]$OutputWav
)
$ErrorActionPreference = 'Stop'
if (Get-Process avesra-desktop -ErrorAction SilentlyContinue) { throw 'Avesra is already running; inspection will not interrupt it.' }
if (![System.IO.Path]::IsPathFullyQualified($OutputWav) -or [System.IO.Path]::GetExtension($OutputWav) -ne '.wav') { throw 'Choose an absolute new .wav evidence path.' }
if ($OutputWav.Contains('"') -or $OutputWav.EndsWith('\')) { throw 'Invalid evidence path.' }
if ((Test-Path -LiteralPath $OutputWav) -or (Test-Path -LiteralPath "$OutputWav.json")) { throw 'Recording paths already exist; choose a new evidence name.' }
if (Get-NetTCPConnection -LocalPort 9475 -State Listen -ErrorAction SilentlyContinue) { throw 'Inspection port 9475 is already owned; do not attach to an unrelated process.' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
# Only use a revision that implements --capture-output. An older binary could
# ignore this argument and greet through the physical speaker. See the runbook.
$Arguments = '--capture-output "' + $OutputWav + '"'
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class AvesraInspectionDesktop {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct Startup {
    public int cb; public string reserved; public string desktop; public string title;
    public uint x,y,width,height,xChars,yChars,fill,flags;
    public ushort show,reservedBytes; public IntPtr reservedPtr,input,output,error;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct ProcessInfo { public IntPtr process,thread; public uint pid,tid; }
  [StructLayout(LayoutKind.Sequential)]
  public struct StartupEx { public Startup startup; public IntPtr attributes; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern IntPtr CreateDesktopW(string name, IntPtr device, IntPtr mode, uint flags, uint access, IntPtr security);
  [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
  [DllImport("user32.dll", SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr handle, int index, System.Text.StringBuilder value, uint bytes, out uint needed);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern bool CreateProcessW(string app, System.Text.StringBuilder command, IntPtr processSecurity, IntPtr threadSecurity, bool inherit, uint flags, IntPtr environment, string cwd, ref StartupEx startup, out ProcessInfo process);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr list, int count, uint flags, ref IntPtr size);
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr list, uint flags, IntPtr attribute, IntPtr value, IntPtr size, IntPtr previous, IntPtr returned);
  [DllImport("kernel32.dll")] static extern void DeleteProcThreadAttributeList(IntPtr list);
  [DllImport("kernel32.dll")] static extern int GetPackageFullName(IntPtr process, ref uint length, IntPtr name);
  [DllImport("kernel32.dll")] static extern uint ResumeThread(IntPtr thread);
  [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr process, uint code);
  [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern Microsoft.Win32.SafeHandles.SafeFileHandle CreateFileW(string path,uint access,uint share,IntPtr security,uint creation,uint flags,IntPtr template);
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetFinalPathNameByHandleW(Microsoft.Win32.SafeHandles.SafeFileHandle handle,System.Text.StringBuilder path,uint length,uint flags);
  static string InputDesktop() {
    IntPtr input=OpenInputDesktop(0, false, 1);
    if(input==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
    try {
      var value=new System.Text.StringBuilder(1024); uint needed;
      if(!GetUserObjectInformationW(input, 2, value, 2048, out needed)) throw new Win32Exception(Marshal.GetLastWin32Error());
      return value.ToString();
    } finally { CloseDesktop(input); }
  }
  static void RequireNormalStore() {
    string path=System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),"com.avesra.desktop","speaker-candidates");
    using(var handle=CreateFileW(path,0,7,IntPtr.Zero,3,0x02000000,IntPtr.Zero)) {
      if(handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
      var physical=new System.Text.StringBuilder(4096);
      if(GetFinalPathNameByHandleW(handle,physical,4096,0)==0) throw new Win32Exception(Marshal.GetLastWin32Error());
      if(!String.Equals(physical.ToString(),"\\\\?\\"+path,StringComparison.OrdinalIgnoreCase))
        throw new Exception("Refusing redirected voice store: "+physical+". Launch this helper through the existing Explorer window's Document.Application.ShellExecute, not Codex's Shell.Application instance.");
      Console.WriteLine("Physical voice store: "+physical);
    }
  }
  public static void Run(string exe, string cwd, string arguments) {
    RequireNormalStore();
    string inputBefore=InputDesktop();
    string name="AvesraInspection-"+Guid.NewGuid().ToString("N");
    if(String.Equals(inputBefore,name,StringComparison.OrdinalIgnoreCase)) throw new Exception("Inspection cannot use the input desktop");
    IntPtr desktop=CreateDesktopW(name, IntPtr.Zero, IntPtr.Zero, 0, 0x10000000, IntPtr.Zero);
    if(desktop==IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
    try {
      IntPtr size=IntPtr.Zero;
      InitializeProcThreadAttributeList(IntPtr.Zero, 1, 0, ref size);
      IntPtr attributes=Marshal.AllocHGlobal(size), policy=Marshal.AllocHGlobal(4);
      bool initialized=false;
      try {
        if(!InitializeProcThreadAttributeList(attributes, 1, 0, ref size)) throw new Win32Exception(Marshal.GetLastWin32Error());
        initialized=true;
        // Documented desktop-app policy: do not inherit Codex's MSIX filesystem view.
        Marshal.WriteInt32(policy, 1);
        if(!UpdateProcThreadAttribute(attributes, 0, (IntPtr)0x20012, policy, (IntPtr)4, IntPtr.Zero, IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
        StartupEx startup=new StartupEx { startup=new Startup { cb=Marshal.SizeOf<StartupEx>(), desktop=name }, attributes=attributes };
        ProcessInfo child;
        var command=new System.Text.StringBuilder("\""+exe+"\" "+arguments);
        if(!CreateProcessW(exe, command, IntPtr.Zero, IntPtr.Zero, false, 0x08080004, IntPtr.Zero, cwd, ref startup, out child)) throw new Win32Exception(Marshal.GetLastWin32Error());
        try {
          uint length=0;
          int packageResult=GetPackageFullName(child.process, ref length, IntPtr.Zero);
          if(packageResult!=15700) { TerminateProcess(child.process, 1); throw new Exception("Inspection still has package identity: "+packageResult); }
          Console.WriteLine("Inspection PID: "+child.pid+"; package identity: none (15700)");
          ResumeThread(child.thread);
          string inputAfter=InputDesktop();
          if(inputBefore!=inputAfter) { TerminateProcess(child.process,1); throw new Exception("Input desktop changed during launch; inspection stopped"); }
          Console.WriteLine("Inspection desktop: "+name+"; input desktop unchanged: "+inputAfter);
          WaitForSingleObject(child.process, 0xffffffff);
        } finally { CloseHandle(child.thread); CloseHandle(child.process); }
      } finally { if(initialized) DeleteProcThreadAttributeList(attributes); Marshal.FreeHGlobal(policy); Marshal.FreeHGlobal(attributes); }
    } finally { CloseDesktop(desktop); }
  }
}
'@
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9475'
[AvesraInspectionDesktop]::Run($Executable, (Split-Path $Executable -Parent), $Arguments)

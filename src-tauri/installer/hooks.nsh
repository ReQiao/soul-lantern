; 灵魂灯笼 NSIS 安装包钩子（tauri.conf.json → bundle.windows.nsis.installerHooks）
;
; 【为什么能在这里定义页面文案和安装步骤】Tauri 的 installer.nsi 模板在**最开头**
; （所有页面和 Section 之前）就 !include 了这个文件，所以：
;   - 这里 !define 的 MUI_WELCOMEPAGE_* 会被紧接着的欢迎页用上；
;   - 这里定义的 Section 会排在模板自带的 Section WebView2 **前面**执行。
; 模板自带的 NSIS_HOOK_PREINSTALL 在 Section Install 里，比 WebView2 那步还晚，弹窗来不及用。
;
; 注意：这个文件必须是 UTF-8 **带 BOM**，否则 NSIS 会按系统代码页读，中文变乱码。
; 发版时把下面的 5.0-rc1 和 src/logic/version.ts 的 DISPLAY_VERSION 一起改。

!define MUI_WELCOMEPAGE_TITLE "欢迎安装灵魂灯笼 5.0-rc1"
!define MUI_WELCOMEPAGE_TEXT "这个向导会带你装好「灵魂灯笼」5.0-rc1（测试版）。$\r$\n$\r$\n安装前请先关闭正在运行的灵魂灯笼。$\r$\n$\r$\n测试版的账号数据可能会在正式版前清空；遇到问题欢迎在软件的「设置 → 反馈问题」里告诉我们。$\r$\n$\r$\n点击「下一步」继续。"
!define MUI_FINISHPAGE_TITLE "灵魂灯笼 5.0-rc1 安装完成"

; 没有 WebView2 时先说清楚再装。
;
; 查法和模板自带的 Section WebView2 一模一样（EdgeUpdate 客户端 GUID 的 pv 值，HKLM / HKCU 都查），
; 这样判断结果才一致：这里说"要装"，后面那一步就一定会去装。
; 点「确定」什么都不做，交给后面模板自带的 downloadBootstrapper 下载安装；点「取消」直接退出。
; 静默安装（/S）不弹窗，照旧由后面那一步处理。
Section "-SoulLanternWebView2Notice"
  ${If} ${RunningX64}
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${Else}
    ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  ${If} $0 == ""
    ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  ${If} $0 == ""
  ${AndIfNot} ${Silent}
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "灵魂灯笼需要微软的 WebView2 组件才能运行，你的电脑上还没有安装。$\r$\n$\r$\n点「确定」自动下载并安装（需要联网，一般 1～2 分钟）；$\r$\n点「取消」退出安装。" IDOK webview2_notice_ok
    Quit
    webview2_notice_ok:
  ${EndIf}
SectionEnd

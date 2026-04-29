import { Trans, useLingui } from "@lingui/react/macro";
import { createFileRoute, Outlet, useLocation, useRouter } from "@tanstack/react-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Globe, Info, Minus, MonitorSmartphone, Settings, X } from "lucide-react";
import { useEffect } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from "@/components/ui/sidebar";
import { isMac } from "@/lib/utils";

function SettingsLayout() {
  const router = useRouter();
  const { pathname } = useLocation();
  const appWindow = getCurrentWindow();
  const { t } = useLingui();
  const navItems = [
    { to: "/settings/general", icon: Settings, label: t`通用` },
    { to: "/settings/network", icon: Globe, label: t`网络` },
    { to: "/settings/devices", icon: MonitorSmartphone, label: t`设备` },
    { to: "/settings/about", icon: Info, label: t`关于` },
  ] as const;

  useEffect(() => {
    const unlisten = listen<string>("navigate", (event) => {
      router.navigate({ to: event.payload });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [router]);

  return (
    <div className="flex h-screen flex-col">
      {/* Title Bar */}
      <header
        data-tauri-drag-region
        className="flex h-10 shrink-0 items-center justify-between border-b border-sidebar-border bg-sidebar px-4"
      >
        <div className={`flex items-center ${isMac ? "pl-17.5" : ""}`} data-tauri-drag-region>
          <h2 className="pl-1 text-sm font-semibold tracking-tight">
            <Trans>设置</Trans>
          </h2>
        </div>
        <div className="flex items-center gap-1">
          {!isMac && (
            <>
              <button
                type="button"
                onClick={() => appWindow.minimize()}
                className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-sidebar-accent"
              >
                <Minus className="h-3.5 w-3.5" />
              </button>
              <button
                type="button"
                onClick={() => appWindow.close()}
                className="flex h-7 w-9 items-center justify-center text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </>
          )}
        </div>
      </header>

      {/* Main Content */}
      <SidebarProvider
        defaultOpen
        className="min-h-0 flex-1 overflow-hidden"
        style={{ "--sidebar-width": "13.75rem" } as React.CSSProperties}
      >
        <Sidebar collapsible="none" className="border-r">
          <SidebarContent className="pt-2">
            <SidebarGroup>
              <SidebarGroupContent>
                <SidebarMenu>
                  {navItems.map((item) => (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton
                        isActive={pathname.startsWith(item.to)}
                        onClick={() => router.navigate({ to: item.to })}
                      >
                        <item.icon />
                        <span>{item.label}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
        </Sidebar>

        {/* Content */}
        <ScrollArea className="flex-1 bg-muted/30 [&_[data-slot=scroll-area-viewport]>div]:flex! [&_[data-slot=scroll-area-viewport]>div]:flex-col [&_[data-slot=scroll-area-viewport]>div]:min-h-full">
          <main className="flex flex-1 flex-col px-8 py-8">
            <Outlet />
          </main>
        </ScrollArea>
      </SidebarProvider>
    </div>
  );
}

export const Route = createFileRoute("/settings")({
  component: SettingsLayout,
});

import { Trans, useLingui } from "@lingui/react/macro";
import { useEffect, useState } from "react";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command";
import { useCommands } from "@/lib/commands";

export const OPEN_COMMAND_PALETTE = "open-command-palette";

export function CommandPalette() {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const { actions, recents } = useCommands();

  useEffect(() => {
    function handleOpen() {
      setOpen(true);
    }
    document.addEventListener(OPEN_COMMAND_PALETTE, handleOpen);
    return () => document.removeEventListener(OPEN_COMMAND_PALETTE, handleOpen);
  }, []);

  async function runCommand(run: () => void | Promise<void>) {
    setOpen(false);
    await run();
  }

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <Command>
        <CommandInput placeholder={t`输入命令...`} />
        <CommandList>
          <CommandEmpty>
            <Trans>没有找到匹配的命令</Trans>
          </CommandEmpty>
          <CommandGroup heading={t`操作`}>
            {actions.map((cmd) => {
              const Icon = cmd.icon;
              return (
                <CommandItem key={cmd.id} onSelect={() => runCommand(cmd.run)}>
                  <Icon className="h-4 w-4" />
                  {cmd.label}
                  {cmd.shortcut && <CommandShortcut>{cmd.shortcut}</CommandShortcut>}
                </CommandItem>
              );
            })}
          </CommandGroup>
          {recents.length > 0 && (
            <CommandGroup heading={t`最近文件`}>
              {recents.map((cmd) => {
                const Icon = cmd.icon;
                return (
                  <CommandItem key={cmd.id} onSelect={() => runCommand(cmd.run)}>
                    <Icon className="h-4 w-4" />
                    {cmd.label}
                  </CommandItem>
                );
              })}
            </CommandGroup>
          )}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

import { Trans } from "@lingui/react/macro";
import { REGEXP_ONLY_DIGITS } from "input-otp";
import { Search } from "lucide-react";
import { useState } from "react";
import { getDeviceByCode } from "@/commands/pairing";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ErrorMessage } from "@/components/ui/error-message";
import { InputOTP, InputOTPGroup, InputOTPSlot } from "@/components/ui/input-otp";
import { useAsyncAction } from "@/hooks/useAsyncAction";

interface InputCodeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDeviceFound: (
    peerId: string,
    osInfo: { hostname: string; os: string; platform: string; arch: string },
    code: string,
  ) => void;
}

export function InputCodeDialog({ open, onOpenChange, onDeviceFound }: InputCodeDialogProps) {
  const [code, setCode] = useState("");
  const { loading, error, run, clearError } = useAsyncAction();

  function handleChange(value: string) {
    setCode(value);
    clearError();
  }

  async function handleSearch() {
    if (code.length < 6) return;
    await run(async () => {
      const result = await getDeviceByCode(code);
      onDeviceFound(result.peerId, result.osInfo, code);
    });
  }

  function handleClose(nextOpen: boolean) {
    if (!nextOpen) {
      setCode("");
      clearError();
    }
    onOpenChange(nextOpen);
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>
            <Trans>输入配对码</Trans>
          </DialogTitle>
          <DialogDescription>
            <Trans>输入对方设备上显示的 6 位数字</Trans>
          </DialogDescription>
        </DialogHeader>

        <div className="flex justify-center py-2">
          <InputOTP
            maxLength={6}
            pattern={REGEXP_ONLY_DIGITS}
            value={code}
            onChange={handleChange}
            onComplete={handleSearch}
          >
            <InputOTPGroup className="gap-2">
              <InputOTPSlot index={0} className="size-11 rounded-md border text-lg" />
              <InputOTPSlot index={1} className="size-11 rounded-md border text-lg" />
              <InputOTPSlot index={2} className="size-11 rounded-md border text-lg" />
              <InputOTPSlot index={3} className="size-11 rounded-md border text-lg" />
              <InputOTPSlot index={4} className="size-11 rounded-md border text-lg" />
              <InputOTPSlot index={5} className="size-11 rounded-md border text-lg" />
            </InputOTPGroup>
          </InputOTP>
        </div>

        <ErrorMessage error={error} className="text-center" />

        <DialogFooter>
          <Button variant="outline" onClick={() => handleClose(false)} disabled={loading}>
            <Trans>取消</Trans>
          </Button>
          <Button onClick={handleSearch} disabled={code.length < 6} loading={loading}>
            {loading ? (
              <Trans>查找中...</Trans>
            ) : (
              <>
                <Search className="h-4 w-4" />
                <Trans>查找设备</Trans>
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

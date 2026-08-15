import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Settings,
  Trash2,
  Send,
  Image as ImageIcon,
  MessageSquare,
  Sparkles,
  Download,
  X,
  Loader2,
  Plus,
  KeyRound,
  FileImage,
  Eye,
  EyeOff,
} from "lucide-react";
import type { ChatMessage, StreamChunkEvent, GeneratedImage } from "./types";
import { SIZE_OPTIONS, RATIO_OPTIONS } from "./types";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------
function uid(): string {
  return crypto.randomUUID ? crypto.randomUUID() : Math.random().toString(36).slice(2);
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/** Single chat message bubble */
function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex w-full mb-4", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[80%] rounded-2xl px-4 py-2.5 text-sm leading-relaxed shadow-sm",
          isUser
            ? "bg-primary text-primary-foreground rounded-br-md"
            : "bg-muted text-foreground rounded-bl-md"
        )}
      >
        {message.content.trim() === "" ? (
          <span className="inline-flex items-center gap-2 text-muted-foreground italic">
            <Loader2 className="size-3.5 animate-spin" />
            思考中…
          </span>
        ) : (
          <div className="prose prose-sm dark:prose-invert max-w-none break-words prose-pre:bg-black/40 prose-pre:border prose-pre:border-border prose-pre:text-foreground">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.content}</ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}

/** Settings dialog for API Key */
function SettingsDialog({
  open,
  onOpenChange,
  apiKey,
  onSave,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  apiKey: string;
  onSave: (key: string) => void;
}) {
  const [value, setValue] = useState(apiKey);
  const [show, setShow] = useState(false);

  // Reset draft to the persisted value each time the dialog opens.
  useEffect(() => {
    if (open) {
      setValue(apiKey);
      setShow(false);
    }
  }, [open, apiKey]);

  const handleSave = () => {
    onSave(value);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <KeyRound className="size-4" />
            设置
          </DialogTitle>
          <DialogDescription>
            配置你的 Agnes API Key，或启动时通过环境变量提供。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="api-key">API Key</Label>
            <div className="relative">
              <Input
                id="api-key"
                type={show ? "text" : "password"}
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleSave();
                }}
                placeholder="sk-xxxxx 或从环境变量 AGNES_API_KEY"
                className="pr-10"
                autoComplete="off"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="absolute top-1/2 right-1 -translate-y-1/2 text-muted-foreground"
                onClick={() => setShow((s) => !s)}
                title={show ? "隐藏" : "显示"}
              >
                {show ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </Button>
            </div>
          </div>

          <div className="rounded-lg bg-muted/60 p-3 text-xs text-muted-foreground">
            <p className="mb-1 font-medium text-foreground/80">或者设置环境变量：</p>
            <code className="block rounded bg-background px-2 py-1 text-[11px] text-emerald-500">
              export AGNES_API_KEY="your-key-here"
            </code>
          </div>

          <Separator />

          <div className="space-y-2">
            <p className="text-xs font-medium text-foreground/80">可用模型</p>
            <ul className="space-y-1 text-xs text-muted-foreground">
              <li className="flex items-center gap-2">
                <Badge variant="secondary" className="font-mono">agnes-2.5-pro</Badge>
                文本对话
              </li>
              <li className="flex items-center gap-2">
                <Badge variant="secondary" className="font-mono">agnes-image-2.1-flash</Badge>
                图像生成
              </li>
            </ul>
          </div>
        </div>

        <DialogFooter>
          <Button onClick={handleSave} className="w-full">
            保存并关闭
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Image Generation panel */
function ImageGenerator({ apiKey }: { apiKey: string }) {
  const [prompt, setPrompt] = useState("");
  const [size, setSize] = useState("2K");
  const [ratio, setRatio] = useState("1:1");
  const [mode, setMode] = useState<"text-to-image" | "image-to-image">("text-to-image");
  const [inputImages, setInputImages] = useState<string[]>([]);
  const [outputFormat, setOutputFormat] = useState<"url" | "base64">("url");
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState("");
  const [generatedImages, setGeneratedImages] = useState<GeneratedImage[]>([]);
  const [error, setError] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const unlisten = listen<string>("image-progress", (event) => {
      try {
        const data = JSON.parse(event.payload as string) as { status: string; message: string };
        if (data.status === "complete") {
          setLoading(false);
          setProgress("");
        } else {
          setProgress(data.message);
        }
      } catch {
        // ignore
      }
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleGenerate = async () => {
    if (!apiKey.trim()) {
      setError("请先配置 API Key");
      return;
    }
    if (!prompt.trim()) {
      setError("请输入提示词");
      return;
    }

    setLoading(true);
    setError("");
    setProgress("生成中...");

    try {
      const result = await invoke("generate_image", {
        args: {
          prompt: prompt.trim(),
          size,
          ratio: ratio !== "1:1" ? ratio : undefined,
          input_images: mode === "image-to-image" ? inputImages : undefined,
          output_format: outputFormat,
        },
      });

      const parsed = typeof result === "string" ? JSON.parse(result) : result;

      const newImage: GeneratedImage = {
        id: uid(),
        prompt: prompt.trim(),
        url: parsed.url,
        base64: parsed.b64_json,
        format: outputFormat,
        size,
        ratio,
        timestamp: Date.now(),
        revisedPrompt: parsed.revised_prompt,
      };

      setGeneratedImages((prev) => [newImage, ...prev]);
      setProgress("");
    } catch (err) {
      setError(String(err));
      setLoading(false);
      setProgress("");
    }
  };

  const handleImageUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    const readers = files.map(
      (file) =>
        new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(reader.result as string);
          reader.onerror = reject;
          reader.readAsDataURL(file);
        })
    );

    Promise.all(readers).then((urls) => {
      setInputImages((prev) => [...prev, ...urls]);
    });
  };

  const removeImage = (index: number) => {
    setInputImages((prev) => prev.filter((_, i) => i !== index));
  };

  const downloadImage = async (image: GeneratedImage) => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");

      if (image.url) {
        await open(image.url);
      } else if (image.base64) {
        const link = document.createElement("a");
        link.href = image.base64;
        link.download = `agnes-image-${image.id}.png`;
        link.click();
      }
    } catch {
      if (image.base64) {
        const link = document.createElement("a");
        link.href = image.base64;
        link.download = `agnes-image-${image.id}.png`;
        link.click();
      }
    }
  };

  return (
    <ScrollArea className="flex-1">
      <div className="mx-auto max-w-2xl space-y-6 p-4">
        {/* Mode Toggle */}
        <Tabs value={mode} onValueChange={(v) => setMode(v as typeof mode)}>
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="text-to-image">
              <Sparkles className="size-4" />
              文生图
            </TabsTrigger>
            <TabsTrigger value="image-to-image">
              <FileImage className="size-4" />
              图生图
            </TabsTrigger>
          </TabsList>
        </Tabs>

        {/* Prompt */}
        <div className="space-y-2">
          <Label>提示词</Label>
          <Textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder={mode === "text-to-image" ? "描述你想生成的图像..." : "描述你要修改的图像..."}
            rows={3}
            className="resize-none"
          />
        </div>

        {/* Reference images (img2img) */}
        {mode === "image-to-image" && (
          <div className="space-y-2">
            <Label>参考图像</Label>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              multiple
              onChange={handleImageUpload}
              className="hidden"
            />
            <Button variant="outline" className="w-full" onClick={() => fileInputRef.current?.click()}>
              <Plus className="size-4" />
              添加图像
            </Button>
            {inputImages.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {inputImages.map((img, idx) => (
                  <div key={idx} className="relative size-20">
                    <img src={img} alt="" className="size-full rounded-lg object-cover ring-1 ring-border" />
                    <button
                      onClick={() => removeImage(idx)}
                      className="absolute -top-2 -right-2 flex size-5 items-center justify-center rounded-full bg-destructive text-destructive-foreground text-xs"
                    >
                      <X className="size-3" />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Size & Ratio */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <Label>尺寸档位</Label>
            <Select value={size} onValueChange={setSize}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SIZE_OPTIONS.map((s) => (
                  <SelectItem key={s} value={s}>
                    {s}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label>宽高比</Label>
            <Select value={ratio} onValueChange={setRatio}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {RATIO_OPTIONS.map((r) => (
                  <SelectItem key={r} value={r}>
                    {r}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {/* Output format */}
        <div className="space-y-2">
          <Label>输出格式</Label>
          <Tabs value={outputFormat} onValueChange={(v) => setOutputFormat(v as typeof outputFormat)}>
            <TabsList className="grid w-full grid-cols-2">
              <TabsTrigger value="url">URL 链接</TabsTrigger>
              <TabsTrigger value="base64">Base64</TabsTrigger>
            </TabsList>
          </Tabs>
        </div>

        {/* Progress */}
        {(loading || progress) && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="size-4 animate-spin" />
            <span>{progress || "生成中..."}</span>
          </div>
        )}

        {/* Error */}
        {error && (
          <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </div>
        )}

        {/* Generate */}
        <Button
          onClick={handleGenerate}
          disabled={loading || !prompt.trim()}
          className="w-full"
          size="lg"
        >
          {loading ? <Loader2 className="size-4 animate-spin" /> : <Sparkles className="size-4" />}
          {loading ? "生成中..." : "生成图像"}
        </Button>

        {/* Gallery */}
        {generatedImages.length > 0 && (
          <div className="space-y-3">
            <h3 className="text-sm font-medium text-muted-foreground">生成历史</h3>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
              {generatedImages.map((img) => (
                <Card key={img.id} className="overflow-hidden">
                  <div className="relative aspect-square bg-muted">
                    {img.url ? (
                      <img src={img.url} alt={img.prompt} className="size-full object-cover" />
                    ) : img.base64 ? (
                      <img src={img.base64} alt={img.prompt} className="size-full object-cover" />
                    ) : (
                      <div className="flex size-full items-center justify-center text-sm text-muted-foreground">
                        无预览
                      </div>
                    )}
                    <Button
                      size="icon-sm"
                      variant="secondary"
                      className="absolute right-2 bottom-2 backdrop-blur"
                      onClick={() => downloadImage(img)}
                    >
                      <Download className="size-4" />
                    </Button>
                  </div>
                  <CardContent className="space-y-1 p-3">
                    <p className="line-clamp-2 text-xs text-foreground/80">{img.prompt}</p>
                    <div className="flex gap-1.5">
                      <Badge variant="outline" className="text-[10px]">
                        {img.size}
                      </Badge>
                      <Badge variant="outline" className="text-[10px]">
                        {img.ratio}
                      </Badge>
                    </div>
                    {img.revisedPrompt && (
                      <p className="text-[11px] text-muted-foreground italic">优化: {img.revisedPrompt}</p>
                    )}
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        )}
      </div>
    </ScrollArea>
  );
}

// ---------------------------------------------------------------------------
// Main App
// ---------------------------------------------------------------------------
type TabType = "chat" | "image";

export default function App() {
  const [activeTab, setActiveTab] = useState<TabType>("chat");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const [apiKey, setApiKeyState] = useState("");
  const apiKeyLoaded = useRef(false);

  useEffect(() => {
    if (apiKeyLoaded.current) return;
    apiKeyLoaded.current = true;
    invoke("get_api_key")
      .then((savedKey: unknown) => {
        setApiKeyState((savedKey as string | null) ?? "");
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<StreamChunkEvent>("chat-stream-chunk", (event) => {
      const data = event.payload;

      if (data.error) {
        console.error("[stream error]", data.error);
        setMessages((prev) =>
          prev.map((m) =>
            m.id === data.messageId ? { ...m, content: `${m.content}\n\n❌ 错误: ${data.error}` } : m
          )
        );
        setLoading(false);
        return;
      }

      setMessages((prev) =>
        prev.map((m) =>
          m.id === data.messageId ? { ...m, content: m.content + data.content } : m
        )
      );

      if (data.done) {
        setLoading(false);
      }
    }).then((_unlisten) => {
      unlisten = _unlisten;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const sendMessage = useCallback(async () => {
    const trimmed = input.trim();
    if (!trimmed || loading) return;

    const userMsg: ChatMessage = { id: uid(), role: "user", content: trimmed };
    const assistantMsgId = uid();

    setMessages((prev) => [...prev, userMsg, { id: assistantMsgId, role: "assistant", content: "" }]);
    setInput("");
    setLoading(true);

    try {
      const history = [
        { role: "system" as const, content: "You are a helpful AI assistant powered by Agnes 2.5 Pro." },
        ...messages.map((m) => ({ role: m.role, content: m.content })),
        { role: "user" as const, content: trimmed },
      ];

      await invoke("send_chat_message", {
        apiKey: apiKey || undefined,
        history,
        messageId: assistantMsgId,
        stream: true,
      });
    } catch (err: unknown) {
      console.error("[invoke error]", err);
      const errMsg = String(err);
      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantMsgId ? { ...m, content: `❌ 请求失败: ${errMsg}` } : m
        )
      );
      setLoading(false);
    }
  }, [input, loading, messages, apiKey]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  const saveApiKey = (newKey: string) => {
    setApiKeyState(newKey);
    invoke("set_api_key", { apiKey: newKey || undefined });
  };

  const clearHistory = () => setMessages([]);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const autoResize = (el: HTMLTextAreaElement) => {
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 160) + "px";
  };

  return (
    <div className="flex h-screen flex-col bg-background text-foreground dark">
      {/* ── Header ─────────────────────────────────────── */}
      <header className="flex shrink-0 items-center justify-between border-b border-border bg-card/50 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
            <Sparkles className="size-5" />
          </div>
          <div>
            <h1 className="font-heading text-sm font-semibold leading-tight">Agnes AI 助手</h1>
            <p className="text-xs text-muted-foreground">Powered by Sapiens AI</p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={clearHistory}
            disabled={messages.length === 0}
            title="清除对话"
          >
            <Trash2 className="size-4" />
          </Button>
          <Button variant="ghost" size="icon-sm" onClick={() => setShowSettings(true)} title="设置">
            <Settings className="size-4" />
          </Button>
        </div>
      </header>

      {/* ── Tabs ──────────────────────────────────────── */}
      <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as TabType)} className="flex flex-1 flex-col overflow-hidden">
        <div className="shrink-0 border-b border-border bg-card/30 px-4">
          <TabsList className="h-11 w-full max-w-md">
            <TabsTrigger value="chat">
              <MessageSquare className="size-4" />
              对话
            </TabsTrigger>
            <TabsTrigger value="image">
              <ImageIcon className="size-4" />
              图像生成
            </TabsTrigger>
          </TabsList>
        </div>

        {/* ── Chat ────────────────────────────────────── */}
        <TabsContent value="chat" className="flex min-h-0 flex-1 flex-col">
          <ScrollArea className="flex-1">
            <div className="mx-auto max-w-3xl space-y-2 px-4 py-4">
              {messages.length === 0 && (
                <div className="flex min-h-[60vh] flex-col items-center justify-center text-center text-muted-foreground">
                  <div className="mb-4 flex size-16 items-center justify-center rounded-2xl bg-primary/10 text-primary">
                    <Sparkles className="size-8" />
                  </div>
                  <p className="text-lg font-medium text-foreground">开始与 Agnes 2.5 Pro 对话</p>
                  <p className="mt-1 text-sm">代码生成 · 推理分析 · 长上下文理解</p>
                  {!apiKey && (
                    <Button variant="outline" className="mt-4" onClick={() => setShowSettings(true)}>
                      <KeyRound className="size-4" />
                      请先配置 API Key
                    </Button>
                  )}
                </div>
              )}

              {messages.map((msg) => (
                <MessageBubble key={msg.id} message={msg} />
              ))}

              <div ref={messagesEndRef} />
            </div>
          </ScrollArea>

          {/* Input */}
          <div className="shrink-0 border-t border-border bg-card/50 px-4 py-3">
            <div className="mx-auto flex max-w-3xl items-end gap-2">
              <Textarea
                ref={textareaRef}
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                onInput={(e) => autoResize(e.target as HTMLTextAreaElement)}
                placeholder="输入消息…（Enter 发送，Shift+Enter 换行）"
                rows={1}
                disabled={loading}
                className="max-h-40 min-h-[44px] flex-1 resize-none"
              />
              <Button
                onClick={sendMessage}
                disabled={loading || !input.trim()}
                size="icon-lg"
                className="shrink-0"
              >
                {loading ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
              </Button>
            </div>
          </div>
        </TabsContent>

        {/* ── Image ───────────────────────────────────── */}
        <TabsContent value="image" className="flex min-h-0 flex-1 flex-col">
          <ImageGenerator apiKey={apiKey} />
        </TabsContent>
      </Tabs>

      <SettingsDialog
        open={showSettings}
        onOpenChange={setShowSettings}
        apiKey={apiKey}
        onSave={saveApiKey}
      />
    </div>
  );
}

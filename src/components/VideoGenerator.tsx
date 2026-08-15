import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Clapperboard,
  Image as ImageIcon,
  Film,
  Plus,
  X,
  Download,
  ExternalLink,
  Loader2,
  Sparkles,
} from "lucide-react";
import {
  VideoMode,
  VideoResolution,
  VideoAspectRatio,
  VideoGenerationParams,
  GeneratedVideo,
  VideoProgressEvent,
  VideoCompleteEvent,
  VIDEO_RESOLUTION_OPTIONS,
  VIDEO_ASPECT_OPTIONS,
  VIDEO_DURATION_OPTIONS,
} from "@/types";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import {
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
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

const MODE_LABELS: Record<VideoMode, string> = {
  "text-to-video": "文生视频",
  "image-to-video": "图生视频",
  keyframes: "关键帧动画",
};

function statusLabel(v: GeneratedVideo): string {
  switch (v.status) {
    case "completed":
      return "已完成";
    case "in_progress":
      return `生成中 ${Math.round(v.progress)}%`;
    case "failed":
      return "失败";
    case "queued":
    default:
      return "排队中";
  }
}

export function VideoGenerator({ apiKey }: { apiKey: string }) {
  const [mode, setMode] = useState<VideoMode>("text-to-video");
  const [prompt, setPrompt] = useState("");
  const [image, setImage] = useState("");
  const [keyframeImages, setKeyframeImages] = useState<string[]>(["", ""]);
  const [resolution, setResolution] = useState<VideoResolution>("720p");
  const [aspectRatio, setAspectRatio] = useState<VideoAspectRatio>("16:9");
  const [durationLabel, setDurationLabel] = useState<string>(
    VIDEO_DURATION_OPTIONS[1].label
  );
  const [negativePrompt, setNegativePrompt] = useState("");
  const [seed, setSeed] = useState("");

  const [videos, setVideos] = useState<GeneratedVideo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    const progressUn = listen<VideoProgressEvent>("video-progress", (e) => {
      const { videoId, status, progress } = e.payload;
      const mapped =
        status === "in_progress" ||
        status === "queued" ||
        status === "completed" ||
        status === "failed"
          ? (status as GeneratedVideo["status"])
          : "queued";
      setVideos((prev) =>
        prev.map((v) =>
          v.id === videoId ? { ...v, status: mapped, progress } : v
        )
      );
    });

    const completeUn = listen<VideoCompleteEvent>("video-complete", (e) => {
      const { videoId, url, remoteUrl, size, seconds, error: completionError } =
        e.payload;
      setVideos((prev) =>
        prev.map((v) =>
          v.id === videoId
            ? completionError
              ? { ...v, status: "failed", error: completionError }
              : { ...v, status: "completed", videoUrl: url, remoteUrl, size, seconds }
            : v
        )
      );
    });

    return () => {
      progressUn.then((f) => f());
      completeUn.then((f) => f());
    };
  }, []);

  const updateKeyframe = (idx: number, value: string) => {
    setKeyframeImages((prev) =>
      prev.map((kf, i) => (i === idx ? value : kf))
    );
  };

  const addKeyframe = () => setKeyframeImages((prev) => [...prev, ""]);
  const removeKeyframe = (idx: number) =>
    setKeyframeImages((prev) => prev.filter((_, i) => i !== idx));

  const handleGenerate = async () => {
    if (!apiKey.trim()) {
      setError("请先配置 API Key");
      return;
    }
    if (!prompt.trim()) {
      setError("请输入提示词");
      return;
    }
    if (loading) return;

    const numFrames =
      VIDEO_DURATION_OPTIONS.find((d) => d.label === durationLabel)
        ?.numFrames ?? VIDEO_DURATION_OPTIONS[1].numFrames;

    const seedNum = seed.trim() === "" ? undefined : Number(seed);
    const finalSeed =
      seedNum !== undefined && !Number.isNaN(seedNum) ? seedNum : undefined;

    const args: VideoGenerationParams = {
      prompt: prompt.trim(),
      mode,
      resolution,
      aspectRatio,
      numFrames,
      frameRate: 24,
      negativePrompt: negativePrompt.trim() || undefined,
      seed: finalSeed,
      ...(mode === "image-to-video"
        ? { image: image.trim() }
        : {}),
      ...(mode === "keyframes"
        ? { keyframeImages: keyframeImages.map((k) => k.trim()).filter(Boolean) }
        : {}),
    };

    setLoading(true);
    setError("");

    try {
      const videoId = await invoke<string>("generate_video", { args });
      const entry: GeneratedVideo = {
        id: videoId,
        prompt: prompt.trim(),
        mode,
        status: "queued",
        progress: 0,
        timestamp: Date.now(),
      };
      setVideos((prev) => [entry, ...prev]);
      setLoading(false);
    } catch (err) {
      setError(String(err));
      setLoading(false);
    }
  };

  const downloadVideo = (url?: string) => {
    if (!url) return;
    try {
      const a = document.createElement("a");
      a.href = url;
      a.download = `agnes-video-${Date.now()}.mp4`;
      document.body.appendChild(a);
      a.click();
      a.remove();
    } catch {
      /* ignore */
    }
  };

  return (
    <ScrollArea className="flex-1">
      <div className="mx-auto max-w-2xl space-y-6 p-4">
        {/* API Key hint */}
        {!apiKey && (
          <div className="rounded-xl border border-border bg-muted/60 p-3 text-sm text-muted-foreground">
            请先配置 API Key 后使用视频生成。
          </div>
        )}

        {/* Mode */}
        <Tabs value={mode} onValueChange={(v) => setMode(v as VideoMode)}>
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="text-to-video">
              <Clapperboard className="size-4" />
              文生视频
            </TabsTrigger>
            <TabsTrigger value="image-to-video">
              <ImageIcon className="size-4" />
              图生视频
            </TabsTrigger>
            <TabsTrigger value="keyframes">
              <Film className="size-4" />
              关键帧动画
            </TabsTrigger>
          </TabsList>
        </Tabs>

        {/* Prompt */}
        <div className="space-y-2">
          <Label>提示词</Label>
          <Textarea
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder="描述你想生成的视频..."
            rows={3}
            className="resize-none"
          />
        </div>

        {/* Reference image (img2video) */}
        {mode === "image-to-video" && (
          <div className="space-y-2">
            <Label>参考图 URL（公网可访问）</Label>
            <Input
              type="text"
              value={image}
              onChange={(e) => setImage(e.target.value)}
              placeholder="https://example.com/ref.jpg"
            />
          </div>
        )}

        {/* Keyframes */}
        {mode === "keyframes" && (
          <div className="space-y-2">
            <Label>关键帧图片 URL（每行一个/逐个添加）</Label>
            <div className="space-y-2">
              {keyframeImages.map((kf, idx) => (
                <div key={idx} className="flex items-center gap-2">
                  <Input
                    type="text"
                    value={kf}
                    onChange={(e) => updateKeyframe(idx, e.target.value)}
                    placeholder={`关键帧 ${idx + 1} URL`}
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => removeKeyframe(idx)}
                    disabled={keyframeImages.length <= 2}
                    title="移除"
                  >
                    <X className="size-4" />
                  </Button>
                </div>
              ))}
            </div>
            <Button
              type="button"
              variant="outline"
              className="w-full"
              onClick={addKeyframe}
            >
              <Plus className="size-4" />
              添加
            </Button>
          </div>
        )}

        {/* Resolution & Aspect */}
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <Label>分辨率</Label>
            <Select
              value={resolution}
              onValueChange={(v) => setResolution(v as VideoResolution)}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {VIDEO_RESOLUTION_OPTIONS.map((r) => (
                  <SelectItem key={r} value={r}>
                    {r}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <Label>宽高比</Label>
            <Select
              value={aspectRatio}
              onValueChange={(v) => setAspectRatio(v as VideoAspectRatio)}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {VIDEO_ASPECT_OPTIONS.map((a) => (
                  <SelectItem key={a} value={a}>
                    {a}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {/* Duration */}
        <div className="space-y-2">
          <Label>时长</Label>
          <Select value={durationLabel} onValueChange={setDurationLabel}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {VIDEO_DURATION_OPTIONS.map((d) => (
                <SelectItem key={d.label} value={d.label}>
                  {d.label}（{d.numFrames} 帧）
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Negative prompt */}
        <div className="space-y-2">
          <Label>反向提示词（可选）</Label>
          <Textarea
            value={negativePrompt}
            onChange={(e) => setNegativePrompt(e.target.value)}
            placeholder="不希望出现的内容..."
            rows={2}
            className="resize-none"
          />
        </div>

        {/* Seed */}
        <div className="space-y-2">
          <Label>随机种子（可选，可复现）</Label>
          <Input
            type="number"
            value={seed}
            onChange={(e) => setSeed(e.target.value)}
            placeholder="留空为随机"
          />
        </div>

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
          {loading ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Sparkles className="size-4" />
          )}
          {loading ? "生成中..." : "生成视频"}
        </Button>

        {/* History */}
        {videos.length > 0 && (
          <>
            <Separator />
            <div className="space-y-3">
              <h3 className="text-sm font-medium text-muted-foreground">
                生成历史
              </h3>
              <div className="space-y-3">
                {videos.map((v) => (
                  <Card key={v.id}>
                    <CardContent className="space-y-3 p-3">
                      {/* Video preview */}
                      {v.videoUrl ? (
                        <video
                          controls
                          src={v.videoUrl}
                          className="w-full rounded-lg"
                        />
                        ) : (
                          <div className="flex h-40 w-full flex-col items-center justify-center gap-1 rounded-lg bg-muted text-center text-sm text-muted-foreground">
                            {v.status === "failed" ? (
                              <span>失败：{v.error ?? "未知错误"}</span>
                            ) : v.status === "completed" ? (
                              <span>已完成，但视频无法预览，请使用上方“打开原链接”</span>
                            ) : (
                              <span>生成中 / 无预览</span>
                            )}
                          </div>
                        )}

                       {/* Controls row */}
                       <div className="flex items-center justify-between gap-2">
                         <div className="flex flex-wrap gap-1.5">
                           <Badge variant="outline" className="text-[10px]">
                             {statusLabel(v)}
                           </Badge>
                           {v.size && (
                             <Badge variant="outline" className="text-[10px]">
                               {v.size}
                             </Badge>
                           )}
                           {v.seconds && (
                             <Badge variant="outline" className="text-[10px]">
                               {v.seconds}
                             </Badge>
                           )}
                           <Badge variant="secondary" className="text-[10px]">
                             {MODE_LABELS[v.mode]}
                           </Badge>
                         </div>
                         <div className="flex items-center gap-1.5">
                           {v.remoteUrl && (
                             <Button
                               type="button"
                               size="icon-sm"
                               variant="ghost"
                               onClick={() => window.open(v.remoteUrl, "_blank")}
                               title="打开原链接"
                             >
                               <ExternalLink className="size-4" />
                             </Button>
                           )}
                           {v.videoUrl && (
                             <Button
                               type="button"
                               size="icon-sm"
                               variant="secondary"
                               onClick={() => downloadVideo(v.videoUrl)}
                               title="下载"
                             >
                               <Download className="size-4" />
                             </Button>
                           )}
                         </div>
                       </div>

                      {/* Progress bar */}
                      {v.status !== "completed" && (
                        <div className="space-y-1">
                          <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
                            <div
                              className="h-full rounded-full bg-primary transition-all"
                              style={{ width: `${v.progress}%` }}
                            />
                          </div>
                          <p className="text-[11px] text-muted-foreground">
                            {Math.round(v.progress)}%
                          </p>
                        </div>
                      )}

                      {/* Prompt */}
                      <p className="line-clamp-2 text-xs text-foreground/80">
                        {v.prompt}
                      </p>
                    </CardContent>
                  </Card>
                ))}
              </div>
            </div>
          </>
        )}
      </div>
    </ScrollArea>
  );
}

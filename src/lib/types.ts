export type SourceInfo = {
  locked: any;
  id: string;
  name: string;
  visible: boolean;
  source_type: string;
  params?: Record<string, string>;
};

export type DemoSource = SourceInfo;

export type AppSettings = {
  root_dir: string | null;
  record_path: string;
  stream_url: string;
  preview_quality?: string;
};

export type SceneInfo = {
  name: string;
  active: boolean;
  locked: boolean;
};

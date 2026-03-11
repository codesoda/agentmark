/** Message types for communication between UI surfaces and the background service worker. */

export interface SaveRequest {
  type: "save";
  url: string;
  title?: string;
  selectedText?: string;
  tags?: string[];
  note?: string;
}

export interface StatusRequest {
  type: "status";
}

export type BackgroundRequest = SaveRequest | StatusRequest;

export interface SaveResponse {
  type: "save";
  success: boolean;
  error?: string;
}

export interface StatusResponse {
  type: "status";
  connected: boolean;
}

export type BackgroundResponse = SaveResponse | StatusResponse;

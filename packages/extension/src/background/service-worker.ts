/// <reference types="chrome" />

chrome.runtime.onInstalled.addListener(() => {
  console.log("[AgentMark] Extension installed");
});

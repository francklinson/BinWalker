<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

interface DeepScanItem {
  layer: number;
  offset: number;
  size: number;
  name: string;
  description: string;
  confidence: number;
  parent_offset: number | null;
  parent_name: string | null;
  source: string;
}

interface ExtractedFile {
  name: string;
  path: string;
  size: number;
  original_offset: number;
  file_type: string;
}

type RiskLevel = "critical" | "high" | "medium" | "low" | "info";

const scanResults = ref<DeepScanItem[]>([]);
const filePath = ref<string>("");
const loading = ref<boolean>(false);
const extractedFiles = ref<ExtractedFile[]>([]);
const showExtracted = ref<boolean>(false);
const searchQuery = ref<string>("");
const filterType = ref<string>("all");
const filterRisk = ref<string>("all");
const filterLayer = ref<number | null>(null);
const sortBy = ref<string>("offset");
const sortOrder = ref<"asc" | "desc">("asc");
const showRawModal = ref<boolean>(false);

const nameToRisk: Record<string, RiskLevel> = {
  "pem": "critical", "rsa": "critical", "openssl": "critical", "aes": "critical",
  "gpg": "critical", "luks": "critical", "certificate": "critical",
  "elf": "high", "pe": "high", "uefi": "high",
  "squashfs": "medium", "jffs2": "medium", "cramfs": "medium", "ubi": "medium",
  "yaffs": "medium", "ext": "medium", "fat": "medium", "ntfs": "medium",
  "romfs": "medium", "cpio": "medium", "uimage": "medium", "trx": "medium",
  "tplink": "medium", "seama": "medium", "jboot": "medium", "dtb": "medium",
  "gzip": "low", "bzip2": "low", "xz": "low", "lzma": "low", "lz4": "low",
  "zstd": "low", "zip": "low", "rar": "low", "7zip": "low", "tar": "low",
  "png": "info", "jpeg": "info", "jpg": "info", "gif": "info", "svg": "info",
  "pdf": "info", "copyright": "info",
};

function getRiskLevel(name: string, description: string): RiskLevel {
  const nameLower = name.toLowerCase();

  for (const [keyword, level] of Object.entries(nameToRisk)) {
    if (nameLower === keyword || nameLower.startsWith(keyword + " ") || nameLower.endsWith(" " + keyword)) {
      return level;
    }
  }

  const descLower = description.toLowerCase();
  const descPatterns: [string, RiskLevel][] = [
    ["private key", "critical"], ["rsa private", "critical"], ["certificate", "critical"],
    ["elf executable", "high"], ["pe executable", "high"], ["uefi", "high"],
    ["squashfs", "medium"], ["jffs2", "medium"], ["cramfs", "medium"],
    ["gzip compressed", "low"], ["bzip2 compressed", "low"], ["xz compressed", "low"],
    ["lzma compressed", "low"], ["zip compressed", "low"],
    ["copyright text", "info"],
  ];
  for (const [pattern, level] of descPatterns) {
    if (descLower.includes(pattern)) return level;
  }

  return "info";
}

function getRiskColor(level: RiskLevel): string {
  switch (level) {
    case "critical": return "#ef4444";
    case "high": return "#f97316";
    case "medium": return "#f59e0b";
    case "low": return "#3b82f6";
    case "info": return "#6b7280";
  }
}

function getRiskLabel(level: RiskLevel): string {
  switch (level) {
    case "critical": return "严重";
    case "high": return "高";
    case "medium": return "中";
    case "low": return "低";
    case "info": return "信息";
  }
}

const resultsWithRisk = computed(() => {
  return scanResults.value.map(r => ({
    ...r,
    risk: getRiskLevel(r.name, r.description)
  }));
});

const riskCounts = computed(() => {
  const counts: Record<RiskLevel, number> = { critical: 0, high: 0, medium: 0, low: 0, info: 0 };
  for (const r of resultsWithRisk.value) {
    counts[r.risk]++;
  }
  return counts;
});

const filteredResults = computed(() => {
  let results = [...resultsWithRisk.value];
  
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase();
    results = results.filter(r => 
      r.name.toLowerCase().includes(query) ||
      r.description.toLowerCase().includes(query)
    );
  }
  
  if (filterType.value !== "all") {
    results = results.filter(r => r.name === filterType.value);
  }
  
  if (filterRisk.value !== "all") {
    results = results.filter(r => r.risk === filterRisk.value);
  }

  if (filterLayer.value !== null) {
    results = results.filter(r => r.layer === filterLayer.value);
  }
  
  const riskOrder: Record<RiskLevel, number> = { critical: 0, high: 1, medium: 2, low: 3, info: 4 };
  
  results.sort((a, b) => {
    let comparison = 0;
    switch (sortBy.value) {
      case "offset":
        comparison = a.offset - b.offset;
        break;
      case "size":
        comparison = a.size - b.size;
        break;
      case "confidence":
        comparison = a.confidence - b.confidence;
        break;
      case "name":
        comparison = a.name.localeCompare(b.name);
        break;
      case "risk":
        comparison = riskOrder[a.risk] - riskOrder[b.risk];
        break;
      case "layer":
        comparison = a.layer - b.layer;
        break;
    }
    return sortOrder.value === "asc" ? comparison : -comparison;
  });
  
  return results;
});

const uniqueTypes = computed(() => {
  const types = new Set(scanResults.value.map(r => r.name));
  return Array.from(types).sort();
});

const layerCounts = computed(() => {
  const counts: Record<number, number> = {};
  for (const r of scanResults.value) {
    counts[r.layer] = (counts[r.layer] || 0) + 1;
  }
  return Object.entries(counts).sort(([a], [b]) => Number(a) - Number(b));
});

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return Math.round(bytes / Math.pow(k, i) * 100) / 100 + " " + sizes[i];
}

function formatOffset(offset: number): string {
  return "0x" + offset.toString(16).toUpperCase().padStart(8, "0");
}

function normalizeConfidence(confidence: number): number {
  return Math.round((confidence / 250) * 100);
}

function getConfidenceColor(confidence: number): string {
  const normalized = normalizeConfidence(confidence);
  if (normalized >= 80) return "#10b981";
  if (normalized >= 60) return "#f59e0b";
  if (normalized >= 40) return "#f97316";
  return "#ef4444";
}

function toggleSort(field: string) {
  if (sortBy.value === field) {
    sortOrder.value = sortOrder.value === "asc" ? "desc" : "asc";
  } else {
    sortBy.value = field;
    sortOrder.value = "asc";
  }
}

function getLayerLabel(layer: number): string {
  if (layer === 0) return "原始固件";
  return `第 ${layer} 层`;
}

function getLayerColor(layer: number): string {
  const colors = ["#00d4ff", "#10b981", "#f59e0b", "#f97316", "#ef4444", "#8b5cf6", "#ec4899", "#14b8a6"];
  return colors[layer % colors.length];
}

async function scanFile() {
  if (!filePath.value) {
    alert("请选择文件");
    return;
  }
  
  loading.value = true;
  showExtracted.value = false;
  try {
    // 1. 深度递归扫描
    const result = await invoke<DeepScanItem[]>("deep_scan", { path: filePath.value });
    scanResults.value = result;

    // 2. 自动提取所有组件
    const outputDir = filePath.value + "_extracted";
    const extracted = await invoke<ExtractedFile[]>("extract_file", { 
      path: filePath.value, 
      outputDir: outputDir 
    });
    extractedFiles.value = extracted;
    if (extracted.length > 0) {
      showExtracted.value = true;
    }
  } catch (error) {
    console.error("扫描失败:", error);
    alert(`扫描失败: ${error}`);
  } finally {
    loading.value = false;
  }
}

async function selectFile() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      multiple: false,
      filters: [{ name: "固件文件", extensions: ["bin", "img", "fw"] }],
    });
    if (selected) {
      filePath.value = selected as string;
    }
  } catch (error) {
    console.error("选择文件失败:", error);
  }
}
</script>

<template>
  <div class="container">
    <h1>BinWalker - 固件分析工具</h1>
    
    <div class="controls">
      <button @click="selectFile">选择文件</button>
      <input 
        type="text" 
        v-model="filePath" 
        placeholder="文件路径" 
        readonly 
      />
      <button @click="scanFile" :disabled="loading">
        {{ loading ? "扫描分析中..." : "扫描" }}
      </button>
    </div>

    <!-- 提取结果栏 -->
    <div v-if="showExtracted" class="extract-banner">
      <span class="extract-icon">&#9889;</span>
      <span>已自动提取 <strong>{{ extractedFiles.length }}</strong> 个组件到 <code>{{ filePath }}_extracted\</code></span>
      <button @click="showExtracted = false" class="extract-dismiss">&times;</button>
    </div>

    <div class="result" v-if="scanResults.length > 0">
      <div class="result-header">
        <h2>扫描结果 ({{ filteredResults.length }} / {{ scanResults.length }})</h2>
        <div class="risk-summary">
          <div 
            v-for="level in (['critical', 'high', 'medium', 'low', 'info'] as RiskLevel[])" 
            :key="level"
            class="risk-badge"
            :class="{ active: filterRisk === level }"
            :style="{ borderColor: getRiskColor(level) }"
            @click="filterRisk = filterRisk === level ? 'all' : level"
          >
            <span class="risk-badge-label">{{ getRiskLabel(level) }}</span>
            <span class="risk-badge-count" :style="'color:' + getRiskColor(level)">
              {{ riskCounts[level] }}
            </span>
          </div>
        </div>
      </div>

      <!-- 层级筛选标签 -->
      <div class="layer-tabs" v-if="layerCounts.length > 1">
        <button class="layer-tab" :class="{ active: filterLayer === null }"
          @click="filterLayer = null">
          全部
        </button>
        <button v-for="[layerStr, count] in layerCounts" :key="layerStr"
          class="layer-tab" :class="{ active: filterLayer === Number(layerStr) }"
          :style="{ borderColor: filterLayer === Number(layerStr) ? getLayerColor(Number(layerStr)) : '' }"
          @click="filterLayer = filterLayer === Number(layerStr) ? null : Number(layerStr)">
          <span class="layer-dot" :style="{ backgroundColor: getLayerColor(Number(layerStr)) }"></span>
          {{ getLayerLabel(Number(layerStr)) }}
          <span class="layer-count">({{ count }})</span>
        </button>
      </div>

      <div class="filters">
        <input 
          type="text" 
          v-model="searchQuery" 
          placeholder="搜索名称或描述..." 
          class="search-input"
        />
        <select v-model="filterType" class="filter-select">
          <option value="all">所有类型</option>
          <option v-for="type in uniqueTypes" :key="type" :value="type">
            {{ type }}
          </option>
        </select>
        <select v-model="filterRisk" class="filter-select">
          <option value="all">所有风险等级</option>
          <option value="critical">严重</option>
          <option value="high">高</option>
          <option value="medium">中</option>
          <option value="low">低</option>
          <option value="info">信息</option>
        </select>
        <button @click="showRawModal = true" class="btn-secondary">
          查看原始数据
        </button>
      </div>

      <!-- 原始数据弹窗 -->
      <div v-if="showRawModal" class="modal-overlay" @click.self="showRawModal = false">
        <div class="modal-content">
          <div class="modal-header">
            <h3>扫描原始数据</h3>
            <button @click="showRawModal = false" class="modal-close">×</button>
          </div>
          <div class="modal-body">
            <pre class="raw-data">{{ JSON.stringify(scanResults, null, 2) }}</pre>
          </div>
        </div>
      </div>

      <div class="table-container">
        <table class="result-table">
          <thead>
            <tr>
              <th @click="toggleSort('layer')" class="sortable">
                层级
                <span v-if="sortBy === 'layer'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th @click="toggleSort('risk')" class="sortable">
                风险
                <span v-if="sortBy === 'risk'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th @click="toggleSort('offset')" class="sortable">
                偏移量
                <span v-if="sortBy === 'offset'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th @click="toggleSort('size')" class="sortable">
                大小
                <span v-if="sortBy === 'size'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th @click="toggleSort('name')" class="sortable">
                类型
                <span v-if="sortBy === 'name'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th>描述</th>
              <th @click="toggleSort('confidence')" class="sortable">
                置信度
                <span v-if="sortBy === 'confidence'" class="sort-icon">
                  {{ sortOrder === 'asc' ? '↑' : '↓' }}
                </span>
              </th>
              <th>来源</th>
            </tr>
          </thead>
          <tbody>
            <tr 
              v-for="(result, index) in filteredResults" 
              :key="index"
              :class="'risk-row-' + result.risk"
            >
              <td>
                <span class="layer-indicator" :style="{ color: getLayerColor(result.layer) }">
                  L{{ result.layer }}
                </span>
              </td>
              <td class="risk-cell">
                <span 
                  class="risk-tag" 
                  :style="{ 
                    backgroundColor: getRiskColor(result.risk),
                    color: result.risk === 'medium' ? '#000' : '#fff'
                  }"
                >
                  {{ getRiskLabel(result.risk) }}
                </span>
              </td>
              <td class="offset-cell">{{ formatOffset(result.offset) }}</td>
              <td class="size-cell">{{ formatSize(result.size) }}</td>
              <td class="name-cell">{{ result.name }}</td>
              <td class="description-cell">{{ result.description }}</td>
              <td class="confidence-cell">
                <div class="confidence-bar">
                  <div 
                    class="confidence-fill" 
                    :style="{ 
                      width: result.confidence + '%',
                      backgroundColor: getConfidenceColor(result.confidence)
                    }"
                  ></div>
                  <span class="confidence-text">{{ normalizeConfidence(result.confidence) }}%</span>
                </div>
              </td>
              <td class="parent-cell">
                <template v-if="result.parent_name">
                  <span class="parent-name">{{ result.parent_name }}</span>
                  <span class="parent-offset">@{{ formatOffset(result.parent_offset!) }}</span>
                </template>
                <span v-else class="parent-none">-</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="filteredResults.length === 0" class="no-results">
        未找到匹配的结果
      </div>
    </div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  font-family: 'Segoe UI', Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 14px;
  color: #e0e0e0;
  background-color: #1a1a2e;
}

body {
  margin: 0;
  padding: 0;
}

.container {
  max-width: 1400px;
  margin: 0 auto;
  padding: 1.5rem 2rem;
  min-height: 100vh;
}

h1 {
  font-size: 1.5rem;
  font-weight: 600;
  color: #00d4ff;
  margin-bottom: 1.5rem;
  letter-spacing: 0.5px;
}

.controls {
  display: flex;
  gap: 0.75rem;
  margin-bottom: 1.5rem;
  align-items: center;
  background: #16213e;
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid #2a2a4a;
}

button {
  padding: 0.5rem 1.25rem;
  cursor: pointer;
  border: 1px solid #00d4ff;
  background: transparent;
  color: #00d4ff;
  border-radius: 6px;
  font-size: 0.9rem;
  font-weight: 500;
  transition: all 0.2s ease;
  white-space: nowrap;
}

button:hover:not(:disabled) {
  background: #00d4ff;
  color: #1a1a2e;
}

button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  border-color: #555;
  color: #555;
}

.controls input[type="text"] {
  flex: 1;
  padding: 0.5rem 0.75rem;
  border: 1px solid #2a2a4a;
  border-radius: 6px;
  background: #0f0f23;
  color: #e0e0e0;
  font-size: 0.9rem;
}

.extract-banner {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.6rem 1rem;
  margin-bottom: 1rem;
  background: rgba(16, 185, 129, 0.1);
  border: 1px solid #10b981;
  border-radius: 8px;
  color: #10b981;
  font-size: 0.9rem;
}

.extract-icon {
  font-size: 1rem;
}

.extract-banner code {
  background: rgba(0, 0, 0, 0.3);
  padding: 0.1rem 0.4rem;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', monospace;
}

.extract-dismiss {
  margin-left: auto;
  background: none;
  border: none;
  color: #10b981;
  font-size: 1.3rem;
  cursor: pointer;
  padding: 0 0.3rem;
  opacity: 0.7;
  line-height: 1;
}

.extract-dismiss:hover {
  opacity: 1;
}

.result {
  margin-top: 1rem;
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
}

.result-header h2 {
  font-size: 1.1rem;
  font-weight: 500;
  color: #b0b0b0;
  margin: 0;
}

.result-header h2 span {
  color: #00d4ff;
  font-weight: 600;
}

.risk-summary {
  display: flex;
  gap: 0.5rem;
}

.risk-badge {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.8rem;
  border: 2px solid;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  transition: all 0.2s;
  opacity: 0.7;
}

.risk-badge:hover {
  opacity: 1;
  transform: translateY(-1px);
}

.risk-badge.active {
  opacity: 1;
  background: rgba(255, 255, 255, 0.05);
}

.risk-badge-label {
  font-size: 0.8rem;
  color: #b0b0b0;
}

.risk-badge-count {
  font-size: 0.9rem;
  font-weight: 600;
}

.filters {
  display: flex;
  gap: 0.75rem;
  margin-bottom: 1rem;
  align-items: center;
}

.search-input {
  flex: 1;
  max-width: 400px;
  padding: 0.5rem 0.75rem;
  border: 1px solid #2a2a4a;
  border-radius: 6px;
  background: #16213e;
  color: #e0e0e0;
  font-size: 0.9rem;
  transition: border-color 0.2s;
}

.search-input:focus {
  outline: none;
  border-color: #00d4ff;
}

.filter-select {
  padding: 0.5rem 0.75rem;
  border: 1px solid #2a2a4a;
  border-radius: 6px;
  background: #16213e;
  color: #e0e0e0;
  font-size: 0.9rem;
  cursor: pointer;
  min-width: 180px;
}

.filter-select:focus {
  outline: none;
  border-color: #00d4ff;
}

.table-container {
  overflow-x: auto;
  border-radius: 8px;
  border: 1px solid #2a2a4a;
  background: #16213e;
}

.result-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
}

.result-table thead {
  background: #0f3460;
}

.result-table th {
  padding: 0.75rem 1rem;
  text-align: left;
  font-weight: 600;
  color: #00d4ff;
  border-bottom: 2px solid #2a2a4a;
  white-space: nowrap;
  user-select: none;
}

.result-table th.sortable {
  cursor: pointer;
  transition: background 0.2s;
}

.result-table th.sortable:hover {
  background: #1a4a7a;
}

.sort-icon {
  margin-left: 0.25rem;
  font-size: 0.8rem;
}

.result-table td {
  padding: 0.6rem 1rem;
  border-bottom: 1px solid #2a2a4a;
  vertical-align: middle;
}

.result-table tbody tr {
  transition: background 0.15s;
}

.result-table tbody tr:hover {
  background: #1a2a4e;
}

.result-table tbody tr:last-child td {
  border-bottom: none;
}

.risk-row-critical {
  background: rgba(239, 68, 68, 0.08);
}

.risk-row-critical:hover {
  background: rgba(239, 68, 68, 0.15);
}

.risk-row-high {
  background: rgba(249, 115, 22, 0.06);
}

.risk-row-high:hover {
  background: rgba(249, 115, 22, 0.12);
}

.risk-cell {
  width: 80px;
}

.risk-tag {
  display: inline-block;
  padding: 0.25rem 0.6rem;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.offset-cell {
  font-family: 'Consolas', 'Monaco', monospace;
  color: #ffd700;
  font-weight: 500;
}

.size-cell {
  font-family: 'Consolas', 'Monaco', monospace;
  color: #98c379;
  white-space: nowrap;
}

.name-cell {
  font-weight: 500;
  color: #e0e0e0;
}

.description-cell {
  color: #888;
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.confidence-cell {
  min-width: 120px;
}

.confidence-bar {
  position: relative;
  width: 100%;
  height: 20px;
  background: #0f0f23;
  border-radius: 10px;
  overflow: hidden;
}

.confidence-fill {
  height: 100%;
  border-radius: 10px;
  transition: width 0.3s ease;
  opacity: 0.8;
}

.confidence-text {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 0.75rem;
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
}

.no-results {
  text-align: center;
  padding: 3rem;
  color: #666;
  font-size: 1rem;
  background: #16213e;
  border-radius: 8px;
  border: 1px solid #2a2a4a;
}

.btn-secondary {
  padding: 0.5rem 1rem;
  border: 1px solid #2a2a4a;
  background: #16213e;
  color: #b0b0b0;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.9rem;
  transition: all 0.2s;
}

.btn-secondary:hover {
  border-color: #00d4ff;
  color: #00d4ff;
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: #1a1a2e;
  border: 1px solid #2a2a4a;
  border-radius: 8px;
  width: 90%;
  max-width: 900px;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem 1.5rem;
  border-bottom: 1px solid #2a2a4a;
}

.modal-header h3 {
  margin: 0;
  color: #00d4ff;
  font-size: 1.1rem;
}

.modal-close {
  background: none;
  border: none;
  color: #888;
  font-size: 1.5rem;
  cursor: pointer;
  padding: 0;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.modal-close:hover {
  background: #2a2a4a;
  color: #fff;
}

.modal-body {
  padding: 1.5rem;
  overflow: auto;
  flex: 1;
}

.raw-data {
  background: #0f0f23;
  border: 1px solid #2a2a4a;
  border-radius: 6px;
  padding: 1rem;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85rem;
  color: #e0e0e0;
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
}

.parent-cell {
  max-width: 200px;
}

.parent-name {
  color: #888;
  font-size: 0.8rem;
  display: block;
}

.parent-offset {
  color: #666;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.75rem;
}

.parent-none {
  color: #555;
}

.layer-tabs {
  display: flex;
  gap: 0.5rem;
  margin-bottom: 1rem;
  flex-wrap: wrap;
}

.layer-tab {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.8rem;
  border: 2px solid #2a2a4a;
  background: transparent;
  color: #b0b0b0;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: all 0.2s;
}

.layer-tab:hover {
  border-color: #555;
  color: #e0e0e0;
}

.layer-tab.active {
  background: rgba(255, 255, 255, 0.05);
  color: #e0e0e0;
}

.layer-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.layer-count {
  color: #888;
  font-size: 0.8rem;
}

.layer-indicator {
  font-family: 'Consolas', 'Monaco', monospace;
  font-weight: 700;
  font-size: 0.85rem;
}
</style>
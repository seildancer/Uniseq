import { createElement, useState } from "react";
import { createRoot } from "react-dom/client";
import { Resizable } from "re-resizable";
import { $prose } from "@milkdown/utils";
import { Plugin } from "prosemirror-state";

function parseImageTitle(title) {
  if (!title?.startsWith("uniseq-image|")) return null;
  const parts = title.split("|");
  return {
    height: parseInt(parts[1], 10) || 0,
    width: parseInt(parts[2], 10) || 0,
    path: parts.slice(3).join("|"),
  };
}

function ResizableImage({ src, alt, width, height, mountKey, onResizeStop }) {
  const [size, setSize] = useState(
    width > 0 && height > 0 ? { width, height } : { width: "auto", height: "auto" }
  );

  return createElement(Resizable, {
    key: mountKey,
    size,
    style: { display: "inline-block", lineHeight: 0 },
    enable: { bottomRight: true, bottomLeft: true, topRight: true, topLeft: true },
    handleStyles: {
      bottomRight: { width: 10, height: 10, right: 0, bottom: 0, borderRadius: "0 0 3px 0" },
      bottomLeft:  { width: 10, height: 10, left:  0, bottom: 0, borderRadius: "0 0 0 3px" },
      topRight:    { width: 10, height: 10, right: 0, top:    0, borderRadius: "0 3px 0 0" },
      topLeft:     { width: 10, height: 10, left:  0, top:    0, borderRadius: "3px 0 0 0" },
    },
    handleClasses: {
      bottomRight: "image-resize-handle",
      bottomLeft:  "image-resize-handle",
      topRight:    "image-resize-handle",
      topLeft:     "image-resize-handle",
    },
    onResize: (_e, _dir, _ref, d) => {
      setSize((s) => ({
        width:  Math.round((typeof s.width  === "number" ? s.width  : width  || 100) + d.width),
        height: Math.round((typeof s.height === "number" ? s.height : height || 100) + d.height),
      }));
    },
    onResizeStop: (_e, _dir, ref) => {
      const w = ref.offsetWidth;
      const h = ref.offsetHeight;
      setSize({ width: w, height: h });
      onResizeStop(w, h);
    },
  }, createElement("img", {
    src,
    alt: alt ?? "",
    style: { width: "100%", height: "100%", display: "block", pointerEvents: "none" },
    draggable: false,
  }));
}

class ImageResizeView {
  constructor(node, view, getPos) {
    this.view = view;
    this.getPos = getPos;
    this.dom = document.createElement("span");
    this.dom.style.display = "inline-block";
    this.root = createRoot(this.dom);
    this._render(node);
  }

  _render(node) {
    const info = parseImageTitle(node.attrs.title);
    this.root.render(createElement(ResizableImage, {
      key: node.attrs.title ?? node.attrs.src,
      src: node.attrs.src ?? "",
      alt: node.attrs.alt ?? "",
      width: info?.width ?? 0,
      height: info?.height ?? 0,
      mountKey: node.attrs.title ?? node.attrs.src,
      onResizeStop: (w, h) => this._commitSize(w, h),
    }));
  }

  _commitSize(w, h) {
    const pos = this.getPos();
    if (pos === undefined) return;
    const node = this.view.state.doc.nodeAt(pos);
    if (!node) return;
    const info = parseImageTitle(node.attrs.title);
    const path = info?.path ?? "";
    this.view.dispatch(
      this.view.state.tr.setNodeMarkup(pos, null, {
        ...node.attrs,
        title: `uniseq-image|${h}|${w}|${path}`,
      })
    );
  }

  update(node) {
    if (node.type.name !== "image") return false;
    this._render(node);
    return true;
  }

  destroy() {
    this.root.unmount();
  }

  stopEvent() {
    return true;
  }

  ignoreMutation() {
    return true;
  }
}

export default $prose(() =>
  new Plugin({
    props: {
      nodeViews: {
        image: (node, view, getPos) => new ImageResizeView(node, view, getPos),
      },
    },
  })
);

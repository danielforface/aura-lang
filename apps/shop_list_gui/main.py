from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path
import tkinter as tk
from tkinter import filedialog, messagebox, ttk

from storage import ShoppingItem, default_data_path, load_items, save_items


class ShopListApp:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("Shop List")
        self.root.minsize(820, 520)

        self.data_path: Path = default_data_path()
        self.items: list[ShoppingItem] = []
        self.selected_id: str | None = None

        self._build_style()
        self._build_menu()
        self._build_layout()

        self._load_from_disk(self.data_path)

        self.root.protocol("WM_DELETE_WINDOW", self._on_close)

    def _build_style(self) -> None:
        style = ttk.Style()
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass

        style.configure("Title.TLabel", font=("Segoe UI", 16, "bold"))
        style.configure("Hint.TLabel", font=("Segoe UI", 10))

    def _build_menu(self) -> None:
        menubar = tk.Menu(self.root)

        file_menu = tk.Menu(menubar, tearoff=False)
        file_menu.add_command(label="Open…", command=self._menu_open)
        file_menu.add_command(label="Save", command=self._menu_save)
        file_menu.add_separator()
        file_menu.add_command(label="Import JSON…", command=self._menu_import)
        file_menu.add_command(label="Export JSON…", command=self._menu_export)
        file_menu.add_separator()
        file_menu.add_command(label="Exit", command=self._on_close)

        menubar.add_cascade(label="File", menu=file_menu)
        self.root.config(menu=menubar)

    def _build_layout(self) -> None:
        outer = ttk.Frame(self.root, padding=14)
        outer.pack(fill=tk.BOTH, expand=True)

        header = ttk.Frame(outer)
        header.pack(fill=tk.X)

        ttk.Label(header, text="Shop List", style="Title.TLabel").pack(side=tk.LEFT)
        self.path_label = ttk.Label(header, text=str(self.data_path), style="Hint.TLabel")
        self.path_label.pack(side=tk.RIGHT)

        body = ttk.Frame(outer)
        body.pack(fill=tk.BOTH, expand=True, pady=(12, 0))

        # Left: list
        left = ttk.Frame(body)
        left.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        columns = ("name", "qty", "notes", "purchased")
        self.tree = ttk.Treeview(left, columns=columns, show="headings", selectmode="browse")
        self.tree.heading("name", text="Item")
        self.tree.heading("qty", text="Qty")
        self.tree.heading("notes", text="Notes")
        self.tree.heading("purchased", text="Done")

        self.tree.column("name", width=220, anchor=tk.W)
        self.tree.column("qty", width=60, anchor=tk.CENTER)
        self.tree.column("notes", width=260, anchor=tk.W)
        self.tree.column("purchased", width=60, anchor=tk.CENTER)

        yscroll = ttk.Scrollbar(left, orient=tk.VERTICAL, command=self.tree.yview)
        self.tree.configure(yscrollcommand=yscroll.set)

        self.tree.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        yscroll.pack(side=tk.RIGHT, fill=tk.Y)

        self.tree.bind("<<TreeviewSelect>>", self._on_select)
        self.tree.bind("<Double-1>", self._on_double_click)

        # Right: editor
        right = ttk.Frame(body, padding=(14, 0, 0, 0))
        right.pack(side=tk.RIGHT, fill=tk.Y)

        editor = ttk.LabelFrame(right, text="Item")
        editor.pack(fill=tk.X)

        ttk.Label(editor, text="Name").grid(row=0, column=0, sticky=tk.W, padx=10, pady=(10, 4))
        ttk.Label(editor, text="Qty").grid(row=1, column=0, sticky=tk.W, padx=10, pady=4)
        ttk.Label(editor, text="Notes").grid(row=2, column=0, sticky=tk.W, padx=10, pady=4)

        self.name_var = tk.StringVar()
        self.qty_var = tk.StringVar(value="1")

        self.name_entry = ttk.Entry(editor, width=34, textvariable=self.name_var)
        self.qty_entry = ttk.Entry(editor, width=12, textvariable=self.qty_var)
        self.notes_text = tk.Text(editor, width=34, height=6, wrap="word")

        self.name_entry.grid(row=0, column=1, sticky=tk.W, padx=10, pady=(10, 4))
        self.qty_entry.grid(row=1, column=1, sticky=tk.W, padx=10, pady=4)
        self.notes_text.grid(row=2, column=1, sticky=tk.W, padx=10, pady=4)

        editor.grid_columnconfigure(1, weight=1)

        actions = ttk.Frame(right)
        actions.pack(fill=tk.X, pady=(12, 0))

        self.add_btn = ttk.Button(actions, text="Add", command=self._add_item)
        self.update_btn = ttk.Button(actions, text="Update", command=self._update_item, state=tk.DISABLED)
        self.delete_btn = ttk.Button(actions, text="Delete", command=self._delete_item, state=tk.DISABLED)
        self.toggle_btn = ttk.Button(actions, text="Toggle Done", command=self._toggle_done, state=tk.DISABLED)

        self.add_btn.pack(fill=tk.X)
        self.update_btn.pack(fill=tk.X, pady=(6, 0))
        self.delete_btn.pack(fill=tk.X, pady=(6, 0))
        self.toggle_btn.pack(fill=tk.X, pady=(6, 0))

        footer = ttk.LabelFrame(right, text="List")
        footer.pack(fill=tk.X, pady=(12, 0))

        ttk.Button(footer, text="Clear Completed", command=self._clear_completed).pack(fill=tk.X, padx=10, pady=(10, 6))
        ttk.Button(footer, text="Clear All", command=self._clear_all).pack(fill=tk.X, padx=10, pady=(0, 10))

        self.status_var = tk.StringVar(value="Ready")
        status = ttk.Label(outer, textvariable=self.status_var)
        status.pack(fill=tk.X, pady=(10, 0))

    # ---- Persistence
    def _load_from_disk(self, path: Path) -> None:
        try:
            self.items = load_items(path)
            self.data_path = path
            self.path_label.config(text=str(self.data_path))
            self._refresh_tree()
            self._set_status(f"Loaded {len(self.items)} item(s)")
        except Exception as e:
            messagebox.showerror("Load failed", str(e))

    def _save_to_disk(self) -> None:
        try:
            save_items(self.data_path, self.items)
            self._set_status("Saved")
        except Exception as e:
            messagebox.showerror("Save failed", str(e))

    # ---- UI helpers
    def _set_status(self, text: str) -> None:
        self.status_var.set(text)

    def _refresh_tree(self) -> None:
        self.tree.delete(*self.tree.get_children())
        for item in self.items:
            done = "✓" if item.purchased else ""
            self.tree.insert("", tk.END, iid=item.id, values=(item.name, item.qty, item.notes, done))

        self._clear_editor(keep_selection=False)

    def _clear_editor(self, keep_selection: bool) -> None:
        self.selected_id = None
        self.name_var.set("")
        self.qty_var.set("1")
        self.notes_text.delete("1.0", tk.END)
        self.update_btn.config(state=tk.DISABLED)
        self.delete_btn.config(state=tk.DISABLED)
        self.toggle_btn.config(state=tk.DISABLED)
        if not keep_selection:
            for sel in self.tree.selection():
                self.tree.selection_remove(sel)

    def _get_notes(self) -> str:
        return self.notes_text.get("1.0", tk.END).strip()

    def _find_item(self, item_id: str) -> ShoppingItem | None:
        for it in self.items:
            if it.id == item_id:
                return it
        return None

    # ---- Events
    def _on_select(self, _evt: object) -> None:
        selection = self.tree.selection()
        if not selection:
            self._clear_editor(keep_selection=True)
            return

        item_id = selection[0]
        it = self._find_item(item_id)
        if not it:
            self._clear_editor(keep_selection=True)
            return

        self.selected_id = item_id
        self.name_var.set(it.name)
        self.qty_var.set(it.qty)
        self.notes_text.delete("1.0", tk.END)
        self.notes_text.insert("1.0", it.notes)

        self.update_btn.config(state=tk.NORMAL)
        self.delete_btn.config(state=tk.NORMAL)
        self.toggle_btn.config(state=tk.NORMAL)

    def _on_double_click(self, _evt: object) -> None:
        # Double-click toggles completion
        if self.tree.selection():
            self._toggle_done()

    # ---- Commands
    def _add_item(self) -> None:
        name = self.name_var.get().strip()
        if not name:
            messagebox.showwarning("Missing name", "Enter an item name")
            return

        item = ShoppingItem.new(name=name, qty=self.qty_var.get(), notes=self._get_notes())
        self.items.append(item)
        self._refresh_tree()
        self._save_to_disk()
        self._set_status("Added")

    def _update_item(self) -> None:
        if not self.selected_id:
            return
        it = self._find_item(self.selected_id)
        if not it:
            return

        name = self.name_var.get().strip()
        if not name:
            messagebox.showwarning("Missing name", "Enter an item name")
            return

        it.name = name
        it.qty = (self.qty_var.get().strip() or "1")
        it.notes = self._get_notes()
        it.touch()

        self._refresh_tree()
        self._save_to_disk()
        self._set_status("Updated")

    def _delete_item(self) -> None:
        if not self.selected_id:
            return

        it = self._find_item(self.selected_id)
        if not it:
            return

        if not messagebox.askyesno("Delete item", f"Delete '{it.name}'?"):
            return

        self.items = [x for x in self.items if x.id != self.selected_id]
        self._refresh_tree()
        self._save_to_disk()
        self._set_status("Deleted")

    def _toggle_done(self) -> None:
        selection = self.tree.selection()
        if not selection:
            return

        item_id = selection[0]
        it = self._find_item(item_id)
        if not it:
            return

        it.purchased = not it.purchased
        it.touch()
        self._refresh_tree()
        self._save_to_disk()
        self._set_status("Toggled")

    def _clear_completed(self) -> None:
        before = len(self.items)
        self.items = [x for x in self.items if not x.purchased]
        removed = before - len(self.items)
        self._refresh_tree()
        self._save_to_disk()
        self._set_status(f"Cleared {removed} completed")

    def _clear_all(self) -> None:
        if not self.items:
            return
        if not messagebox.askyesno("Clear all", "Remove all items?"):
            return
        self.items = []
        self._refresh_tree()
        self._save_to_disk()
        self._set_status("Cleared all")

    # ---- Menu
    def _menu_open(self) -> None:
        path_str = filedialog.askopenfilename(
            title="Open shopping list",
            filetypes=[("JSON", "*.json"), ("All files", "*")],
        )
        if not path_str:
            return
        self._load_from_disk(Path(path_str))

    def _menu_save(self) -> None:
        self._save_to_disk()

    def _menu_export(self) -> None:
        path_str = filedialog.asksaveasfilename(
            title="Export shopping list",
            defaultextension=".json",
            filetypes=[("JSON", "*.json")],
        )
        if not path_str:
            return

        try:
            export_path = Path(path_str)
            payload = {
                "version": 1,
                "items": [asdict(i) for i in self.items],
            }
            export_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
            self._set_status("Exported")
        except Exception as e:
            messagebox.showerror("Export failed", str(e))

    def _menu_import(self) -> None:
        path_str = filedialog.askopenfilename(
            title="Import shopping list",
            filetypes=[("JSON", "*.json"), ("All files", "*")],
        )
        if not path_str:
            return

        try:
            import_items = load_items(Path(path_str))
            if not import_items:
                messagebox.showinfo("Import", "No items found")
                return

            if self.items and not messagebox.askyesno(
                "Import",
                f"Replace current list with {len(import_items)} imported item(s)?",
            ):
                return

            self.items = import_items
            self._refresh_tree()
            self._save_to_disk()
            self._set_status("Imported")
        except Exception as e:
            messagebox.showerror("Import failed", str(e))

    def _on_close(self) -> None:
        try:
            self._save_to_disk()
        finally:
            self.root.destroy()


def main() -> None:
    root = tk.Tk()
    app = ShopListApp(root)
    _ = app
    root.mainloop()


if __name__ == "__main__":
    main()

"""Headless hostile-input tests of the actual DEV01 AT-SPI selection helper.

Synthetic providers here test selection policy only; they cannot certify a
real installed accessibility tree, OS consent, or file-operation result.
"""
import ast
from pathlib import Path
import subprocess
from types import SimpleNamespace
import unittest

SOURCE = Path(__file__).with_name('dev01_modern_linux_installed_atspi.py')
FUNCTIONS = {'_showing', '_confirmation_nodes', '_native_process_id',
             '_same_window_geometry', '_exact_accessible_text', 'owned_confirmation'}
FLAGS = SimpleNamespace(STATE_VISIBLE=1, STATE_SHOWING=2, STATE_ENABLED=3,
                        STATE_SENSITIVE=4, DESKTOP_COORDS=0)
GEOMETRY = (100, 100, 400, 200)
TITLE = 'Lu IDE · Confirm source file action'
MESSAGE = 'Move move-me.rs to renamed.rs? No existing file will be overwritten.'


class Node:
    def __init__(self, role, name='', pid=42, geometry=GEOMETRY, content='',
                 states=(1, 2, 3, 4), actions=('click',)):
        self.role, self.name, self.pid = role, name, pid
        self.geometry, self.content, self.states = geometry, content, states
        self.actions = actions
        self.children, self.parent, self.calls = [], None, 0

    def add(self, *children):
        for child in children:
            child.parent = self
            self.children.append(child)
        return self

    def getRoleName(self): return self.role
    def getState(self): return SimpleNamespace(contains=lambda flag: flag in self.states)
    def get_process_id(self): return self.pid
    def getApplication(self):
        node = self
        while node.parent is not None:
            node = node.parent
        return node
    def queryComponent(self):
        x, y, width, height = self.geometry
        return SimpleNamespace(getExtents=lambda _: SimpleNamespace(
            x=x, y=y, width=width, height=height))
    def queryText(self):
        return SimpleNamespace(characterCount=len(self.content),
                               getText=lambda start, end: self.content[start:end])
    def queryAction(self):
        def submit(index):
            self.calls += 1
            return index == 0
        return SimpleNamespace(nActions=len(self.actions),
                               getName=lambda i: self.actions[i], doAction=submit)


def make_root(pid=42, geometry=GEOMETRY, message=MESSAGE, role='frame',
              button_name='Apply file action', states=(1, 2, 3, 4)):
    button = Node('push button', button_name, pid=pid, states=states)
    root = Node(role, '', pid, geometry).add(Node('label', content=message, pid=pid), button)
    Node('application', pid=pid).add(root)
    return root, button


def harness(roots, active=None):
    module = ast.parse(SOURCE.read_text(encoding='utf-8'))
    module.body = [node for node in module.body if isinstance(node, ast.FunctionDef)
                   and node.name in FUNCTIONS]
    assert {node.name for node in module.body} == FUNCTIONS
    clock = iter(range(1000))
    def deny(reason): raise RuntimeError(reason)
    ns = dict(pyatspi=FLAGS, native=SimpleNamespace(elements=lambda: iter(roots),
              children_of=lambda node: iter(node.children)), deny=deny,
              time=SimpleNamespace(monotonic=lambda: next(clock), sleep=lambda _: None),
              subprocess=subprocess, print=lambda *args, **kwargs: None, _window_geometry=lambda _: GEOMETRY,
              _active_confirmation_window=active or (lambda proc, title: ('123', 42, title)))
    exec(compile(module, str(SOURCE), 'exec'), ns)
    return ns['owned_confirmation']


class ModalScopeTests(unittest.TestCase):
    def test_unnamed_frame_and_text_interface_dispatch_once(self):
        root, button = make_root()
        harness([root])(None, MESSAGE)
        self.assertEqual(button.calls, 1)

    def test_wrong_pid_wrong_geometry_wrong_message_and_hidden_button_deny(self):
        for kwargs in ({'pid': 97}, {'geometry': (700, 100, 400, 200)},
                       {'message': MESSAGE + ' altered'}, {'states': (1, 3, 4)},
                       {'button_name': 'Cancel'}):
            with self.subTest(kwargs=kwargs):
                root, button = make_root(**kwargs)
                with self.assertRaisesRegex(RuntimeError, 'confirmation_exact_owned'):
                    harness([root])(None, MESSAGE)
                self.assertEqual(button.calls, 0)

    def test_duplicate_owned_modal_denies(self):
        a, x = make_root(); b, y = make_root()
        with self.assertRaises(RuntimeError): harness([a, b])(None, MESSAGE)
        self.assertEqual(x.calls + y.calls, 0)

    def test_nested_webview_dialog_denies(self):
        root, button = make_root(role='dialog')
        Node('document web').add(root)
        with self.assertRaises(RuntimeError): harness([root])(None, MESSAGE)
        self.assertEqual(button.calls, 0)

    def test_duplicate_button_denies(self):
        root, first = make_root()
        second = Node('push button', 'Apply file action')
        root.add(second)
        with self.assertRaises(RuntimeError): harness([root])(None, MESSAGE)
        self.assertEqual(first.calls + second.calls, 0)

    def test_focus_changes_before_dispatch_deny(self):
        root, button = make_root()
        windows = iter([('123', 42, TITLE), ('124', 42, TITLE)])
        with self.assertRaisesRegex(RuntimeError, 'confirmation_native_window_changed'):
            harness([root], lambda proc, title: next(windows))(None, MESSAGE)
        self.assertEqual(button.calls, 0)

    def test_layout_confirmation_requires_own_caption_and_message(self):
        layout = 'Remember file tabs and pane layout only?'
        root, button = make_root(message=layout, button_name='Remember navigation')
        harness([root])(None, layout, 'Lu IDE · Remember tab layout', 'Remember navigation')
        self.assertEqual(button.calls, 1)

    def test_ambiguous_action_denies_without_click(self):
        root, button = make_root()
        button.actions = ('click', 'press')
        with self.assertRaisesRegex(RuntimeError, 'confirmation_native_action_ambiguous'):
            harness([root])(None, MESSAGE)
        self.assertEqual(button.calls, 0)


class RelativeFileOpenScopeTests(unittest.TestCase):
    def exercise(self, nodes, filename='second.rs', change=False):
        module = ast.parse(SOURCE.read_text(encoding='utf-8'))
        module.body = [node for node in module.body if isinstance(node, ast.FunctionDef)
                       and node.name == 'open_exact_relative_fixture_file']
        self.assertEqual(len(module.body), 1)
        flags = SimpleNamespace(STATE_VISIBLE=1, STATE_SHOWING=2, STATE_SELECTED=5)
        calls = []
        digests = iter(('main', 'second', 'main', 'changed' if change else 'second'))

        def deny(reason):
            raise RuntimeError(reason)

        def wait(predicate, reason):
            if not predicate():
                deny(reason)

        ns = dict(pyatspi=flags, native=SimpleNamespace(
            key=lambda key: calls.append(key),
            _native_source_editor_nodes=lambda: iter(nodes)),
            SOURCE='main', SECOND='second', digest=lambda _: next(digests),
            enter=lambda label, value: calls.append((label, value)),
            wait_until=wait, deny=deny)
        exec(compile(module, str(SOURCE), 'exec'), ns)
        ns['open_exact_relative_fixture_file'](filename)
        return calls

    def test_exact_selected_tab_opens_with_native_keyboard(self):
        for filename in ('main.rs', 'second.rs'):
            with self.subTest(filename=filename):
                selected = Node('page tab', 'Saved source tab ' + filename, states=(1, 2, 5))
                self.assertEqual(self.exercise([selected], filename), [
                    ('Open project-relative file', filename), 'Return'])

    def test_wrong_hidden_unselected_and_duplicate_tabs_deny(self):
        selected = Node('page tab', 'Saved source tab second.rs', states=(1, 2, 5))
        cases = [[], [selected, selected],
                 [Node('page tab', 'Saved source tab not-second.rs', states=(1, 2, 5))],
                 [Node('push button', selected.name, states=(1, 2, 5))],
                 [Node('page tab', selected.name, states=(1, 5))],
                 [Node('page tab', selected.name, states=(1, 2))]]
        for nodes in cases:
            with self.subTest(nodes=nodes), self.assertRaisesRegex(RuntimeError, 'not_selected'):
                self.exercise(nodes)

    def test_unexpected_disk_change_denied(self):
        selected = Node('page tab', 'Saved source tab second.rs', states=(1, 2, 5))
        with self.assertRaisesRegex(RuntimeError, 'changed_fixture'):
            self.exercise([selected], change=True)

    def test_non_fixture_path_denied(self):
        with self.assertRaisesRegex(RuntimeError, 'invalid_relative'):
            self.exercise([], '../second.rs')


if __name__ == '__main__':
    unittest.main()

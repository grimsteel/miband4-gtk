use gtk::{glib::{self, Object}, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager, Widget, Window};

glib::wrapper! {
    pub struct AuthKeyDialog(ObjectSubclass<imp::AuthKeyDialog>)
        // https://docs.gtk.org/gtk4/class.Window.html#hierarchy
        @extends Window, Widget,
        @implements Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager;
}

impl AuthKeyDialog {
    pub fn new() -> Self {
        Object::builder().build()
    }
}

mod imp {
    use std::{cell::RefCell, sync::OnceLock};

    use gtk::{glib::{self, subclass::{InitializingObject, Signal}, Properties}, prelude::*, subclass::prelude::*, template_callbacks, Button, CompositeTemplate, Entry, TemplateChild, Window};

    use crate::utils::is_hex_string;

    #[derive(CompositeTemplate, Default, Properties)]
    #[template(resource = "/me/grimsteel/miband4-gtk/auth_key_dialog.ui")]
    #[properties(wrapper_type = super::AuthKeyDialog)]
    pub struct AuthKeyDialog {
        #[template_child]
        entry_auth_key: TemplateChild<Entry>,
        #[property(get, set)]
        pub auth_key: RefCell<String>
    }

    #[template_callbacks]
    impl AuthKeyDialog {
        #[template_callback]
        fn handle_auth_key_cancel(&self, _button: &Button) {
            self.obj().close();
        }
        #[template_callback]
        fn handle_auth_key_save(&self, _button: &Button) {
            self.entry_auth_key.remove_css_class("error");
            
            // validate the value of the input
            let value = self.get_entered_key();
            if value.len() == 32 && is_hex_string(&value) {
                self.obj().emit_by_name::<()>("new-auth-key", &[&value]);
                self.obj().close();
            } else {
                // reflect that in the state of the entry
                self.entry_auth_key.add_css_class("error");
            }
        }
        fn get_entered_key(&self) -> String {
            self.entry_auth_key.buffer().text().as_str().to_string()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AuthKeyDialog {
        const NAME: &'static str = "MiBand4AuthKeyDialog";
        type Type = super::AuthKeyDialog;
        type ParentType = Window;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AuthKeyDialog {
        fn constructed(&self) {
            self.parent_constructed();

            // update the entry's contents when auth_key is changed
            self.obj().connect_auth_key_notify(|win| {
                win.imp().entry_auth_key.buffer().set_text(win.auth_key());
            });

            self.obj().connect_show(|win| {
                win.imp().entry_auth_key.remove_css_class("error");
            });
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("new-auth-key").param_types([String::static_type()]).build()
                ]
            })
        }
    }
    impl WidgetImpl for AuthKeyDialog {}
    impl WindowImpl for AuthKeyDialog {}
}

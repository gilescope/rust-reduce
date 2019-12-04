// Copyright (c) Jethro G. Beekman
//
// This file is part of rust-reduce.
//
// rust-reduce is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-reduce is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-reduce.  If not, see <https://www.gnu.org/licenses/>.

/// Removes `pub`.
/// Named after maggy - thanks Ivan.
pub fn privatise_items<F: FnMut(&syn::File) -> Result<(),String>> (file: &mut syn::File, mut try_compile: F) {
    let mut level = 0;
    let mut index = 0;
    loop {
        let backup = file.clone();
        if !file.items.privatise(level, &mut { index }) {
            if index == 0 {
                break;
            }
            level += 1;
            index = 0;
            continue;
        }
        if let Err(_msg) = try_compile(&file) {
            *file = backup;
            index += 1;
        } else {
            // try delete next, which will be at same index now that we've
            // deleted something
        }
    }
}

trait Privatise {
    fn privatise(&mut self, level: usize, index: &mut usize) -> bool;
}

fn privatise_item(item: &mut Option<&mut syn::Item>) -> bool {
    match item {
        Some(syn::Item::Fn(fun)) => {
            if let syn::Visibility::Inherited = fun.vis {
                //private already
            } else {
                fun.vis = syn::Visibility::Inherited;
                return true;
            }
        },
        _ => {}
    }
    false
}


impl Privatise for Vec<syn::Item> {
    fn privatise(&mut self, level: usize, index: &mut usize) -> bool {
        if level == 0 {
            if *index < self.len() {
                privatise_item(&mut self.get_mut(*index))
            } else {
                *index -= self.len();
                false
            }
        } else {
            for item in self {
                if match item {
                    syn::Item::Mod(syn::ItemMod {
                        content: Some((_, items)),
                        ..
                    }) => items.privatise(level - 1, index),
                    syn::Item::Struct(item @ syn::ItemStruct{ .. }) => item.privatise(level - 1, index),
                    syn::Item::Impl(syn::ItemImpl { items, .. }) => items.privatise(level - 1, index),
                    syn::Item::Enum(item @ syn::ItemEnum{ .. }) => item.privatise(level - 1, index),
                    _ => false,
                } {
                    return true;
                }
            }
            false
        }
    }
}

impl Privatise for Vec<syn::ImplItem> {
    fn privatise(&mut self, level: usize, index: &mut usize) -> bool {
        if level < 5 {
            if *index < self.len() {
                if let Some(syn::ImplItem::Method(meth))
                    = &mut self.get_mut(*index) {
                    if let syn::Visibility::Inherited = meth.vis {
                        false
                    } else {
                        meth.vis = syn::Visibility::Inherited;
                        true
                    }
                } else { false }
            } else {
                *index -= self.len();
                false
            }
        } else {
            false
        }
    }
}

impl Privatise for syn::ItemEnum{
    fn privatise(&mut self, _level: usize, _index: &mut usize) -> bool {
//        if level < 5 {
//            if *index < self.variants.len() {
//                for (i, var) in self.variants.iter_mut().enumerate() {
//                    if i == *index {
//                        var.fields.each field could be pub
//                        return privatise_item(&mut Some(var));
//                    }
//                }
//            }
//            else {
//                *index -= self.variants.len();
//            }
//        }
        false
    }
}

impl Privatise for syn::ItemStruct {
    fn privatise(&mut self, level: usize, index: &mut usize) -> bool {
        if level < 5 {
            if let syn::Fields::Named(ref mut named) = self.fields {
                if *index < named.named.len() {
                    for (i, field) in &mut self.fields.iter_mut().enumerate()
                    {
                        if i == *index {
                            return if let syn::Visibility::Inherited = field.vis {
                                false
                            } else {
                                field.vis = syn::Visibility::Inherited;
                                true
                            };
                        }
                    }
                    false
                }
                else {
                    *index -= named.named.len();
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}
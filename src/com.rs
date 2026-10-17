//! Thin layer over COM automation objects (late binding through IDispatch), used to drive EA.
//!
//! Values returned by EA are read directly from the VARIANT fields. The ready-made conversions of the `windows` crate
//! (bool::try_from, i32::try_from, to_string) call functions from propsys.dll (VariantToBoolean, VariantToInt32,
//! PropVariantToBSTR), which Wine on macOS does not have: the program terminates there immediately.

use anyhow::{anyhow, bail, Result};
use windows::core::{Interface, GUID, HSTRING, PCWSTR, VARIANT};
use windows::Win32::System::Com::{
    IDispatch, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, EXCEPINFO,
};
use windows::Win32::System::Ole::DISPID_PROPERTYPUT;

const VT_EMPTY: u16 = 0;
const VT_NULL: u16 = 1;
const VT_I2: u16 = 2;
const VT_I4: u16 = 3;
const VT_BSTR: u16 = 8;
const VT_DISPATCH: u16 = 9;
const VT_BOOL: u16 = 11;
const VT_UI1: u16 = 17;
const VT_UI2: u16 = 18;
const VT_UI4: u16 = 19;
const VT_INT: u16 = 22;
const VT_UINT: u16 = 23;

/// Value returned by a COM object.
pub struct Var(VARIANT);

impl Var {
    fn kind(&self) -> u16 {
        unsafe { self.0.as_raw().Anonymous.Anonymous.vt as u16 }
    }

    /// Value type code (VT_*), for diagnostics.
    #[allow(dead_code)]
    pub fn kind_code(&self) -> u16 {
        self.kind()
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.kind(), VT_EMPTY | VT_NULL)
    }

    pub fn bool(&self) -> bool {
        let raw = self.0.as_raw();
        unsafe {
            match self.kind() {
                VT_BOOL => raw.Anonymous.Anonymous.Anonymous.boolVal != 0,
                VT_I2 | VT_UI2 => raw.Anonymous.Anonymous.Anonymous.iVal != 0,
                VT_I4 | VT_UI4 | VT_INT | VT_UINT => raw.Anonymous.Anonymous.Anonymous.lVal != 0,
                _ => false,
            }
        }
    }

    pub fn i32(&self) -> i32 {
        let raw = self.0.as_raw();
        unsafe {
            match self.kind() {
                VT_I4 | VT_UI4 | VT_INT | VT_UINT => raw.Anonymous.Anonymous.Anonymous.lVal,
                VT_I2 | VT_UI2 => raw.Anonymous.Anonymous.Anonymous.iVal as i32,
                VT_UI1 => raw.Anonymous.Anonymous.Anonymous.bVal as i32,
                VT_BOOL => (raw.Anonymous.Anonymous.Anonymous.boolVal != 0) as i32,
                // Numbers stored as text (EA returns e.g. some identifiers this way).
                VT_BSTR => self.text().trim().parse().unwrap_or(0),
                _ => 0,
            }
        }
    }

    pub fn text(&self) -> String {
        match self.kind() {
            VT_BSTR => unsafe {
                let ptr = self.0.as_raw().Anonymous.Anonymous.Anonymous.bstrVal as *const u16;
                if ptr.is_null() {
                    return String::new();
                }
                // The BSTR length is stored in the 4 bytes before the text (in bytes, without the terminating zero).
                let bytes = *(ptr as *const u32).offset(-1) as usize;
                String::from_utf16_lossy(std::slice::from_raw_parts(ptr, bytes / 2))
            },
            VT_I2 | VT_I4 | VT_UI1 | VT_UI2 | VT_UI4 | VT_INT | VT_UINT => self.i32().to_string(),
            VT_BOOL => self.bool().to_string(),
            _ => String::new(),
        }
    }

    pub fn object(&self) -> Option<Disp> {
        if self.kind() != VT_DISPATCH {
            return None;
        }
        unsafe {
            let ptr = self.0.as_raw().Anonymous.Anonymous.Anonymous.pdispVal;
            IDispatch::from_raw_borrowed(&ptr).cloned().map(Disp)
        }
    }
}

/// Automation object (Repository, Package, Element, collection…).
#[derive(Clone)]
pub struct Disp(pub IDispatch);

impl Disp {
    fn invoke(&self, name: &str, flags: DISPATCH_FLAGS, args: &[VARIANT]) -> Result<Var> {
        unsafe {
            let wide = HSTRING::from(name);
            let names = [PCWSTR(wide.as_ptr())];
            let mut id = 0i32;
            self.0
                .GetIDsOfNames(&GUID::zeroed(), names.as_ptr(), 1, 0, &mut id)
                .map_err(|e| anyhow!("{}", crate::i18n::fmt("EA does not know \"{0}\": {1}", &[name.to_string(), e.to_string()])))?;
            // IDispatch takes the arguments in reverse order.
            let mut values: Vec<VARIANT> = args.iter().rev().cloned().collect();
            let mut named = [DISPID_PROPERTYPUT];
            let put = flags == DISPATCH_PROPERTYPUT;
            let params = DISPPARAMS {
                rgvarg: values.as_mut_ptr(),
                rgdispidNamedArgs: if put { named.as_mut_ptr() } else { std::ptr::null_mut() },
                cArgs: values.len() as u32,
                cNamedArgs: if put { 1 } else { 0 },
            };
            let mut result = VARIANT::default();
            // Full structures for the error description: cross-process call marshalling in Wine does not like null pointers.
            let mut exception = EXCEPINFO::default();
            let mut bad_argument = 0u32;
            self.0
                .Invoke(id, &GUID::zeroed(), 0, flags, &params, Some(&mut result), Some(&mut exception), Some(&mut bad_argument))
                .map_err(|e| {
                    let detail = exception.bstrDescription.to_string();
                    if detail.trim().is_empty() {
                        anyhow!("{name}: {e}")
                    } else {
                        anyhow!("{name}: {}", detail.trim())
                    }
                })?;
            Ok(Var(result))
        }
    }

    /// Method call.
    pub fn call(&self, name: &str, args: &[VARIANT]) -> Result<Var> {
        self.invoke(name, DISPATCH_METHOD, args)
    }

    /// Property read.
    pub fn get(&self, name: &str) -> Result<Var> {
        self.invoke(name, DISPATCH_PROPERTYGET, &[])
    }

    /// Property write.
    pub fn set(&self, name: &str, value: VARIANT) -> Result<()> {
        self.invoke(name, DISPATCH_PROPERTYPUT, &[value]).map(|_| ())
    }

    /// Property that is an object (e.g. the Packages collection).
    pub fn obj(&self, name: &str) -> Result<Disp> {
        self.get(name)?.object().ok_or_else(|| anyhow!("{}", crate::i18n::fmt("{0}: EA returned no object", &[name.to_string()])))
    }

    /// Method returning an object (e.g. AddNew, GetAt, GetPackageByID).
    pub fn call_obj(&self, name: &str, args: &[VARIANT]) -> Result<Disp> {
        self.call(name, args)?.object().ok_or_else(|| anyhow!("{}", crate::i18n::fmt("{0}: EA returned no object", &[name.to_string()])))
    }

    pub fn int(&self, name: &str) -> Result<i32> {
        Ok(self.get(name)?.i32())
    }

    pub fn text(&self, name: &str) -> Result<String> {
        Ok(self.get(name)?.text())
    }

    /// Update() of an EA object; on failure, the error text from GetLastError().
    pub fn update(&self, what: &str) -> Result<()> {
        if !self.call("Update", &[])?.bool() {
            let detail = self.call("GetLastError", &[]).map(|v| v.text()).unwrap_or_default();
            bail!("{}", crate::i18n::fmt("save failed ({0}): {1}", &[what.to_string(), detail]));
        }
        Ok(())
    }

    /// Items of an EA collection (Count / GetAt). The index is passed as a 16-bit number, as in the EA declaration.
    pub fn items(&self) -> Result<Vec<Disp>> {
        let count = self.int("Count")?;
        // EA takes a 16-bit index; a 32-bit one is converted by COM and fails cleanly above 32767 instead of wrapping around.
        (0..count).map(|i| self.call_obj("GetAt", &[VARIANT::from(i as i32)])).collect()
    }
}

/// Argument values.
pub fn s(text: &str) -> VARIANT {
    VARIANT::from(text)
}

pub fn i(number: i32) -> VARIANT {
    VARIANT::from(number)
}

pub fn b(value: bool) -> VARIANT {
    VARIANT::from(value)
}

#[allow(dead_code)]
pub fn as_dispatch<T: Interface>(object: &T) -> Result<Disp> {
    Ok(Disp(object.cast::<IDispatch>()?))
}

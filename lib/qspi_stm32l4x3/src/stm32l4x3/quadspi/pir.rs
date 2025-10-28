#![allow(non_camel_case_types)]

use crate::stm32l4x3::{
    field_reader::FieldReader,
    generics::{Readable, RegisterSpec, Resettable, Writable, R as hR, W as hW},
};

#[doc = "Register `PIR` reader"]
pub struct R(hR<PIR_SPEC>);
impl core::ops::Deref for R {
    type Target = hR<PIR_SPEC>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<hR<PIR_SPEC>> for R {
    #[inline(always)]
    fn from(reader: hR<PIR_SPEC>) -> Self {
        R(reader)
    }
}
#[doc = "Register `PIR` writer"]
pub struct W(hW<PIR_SPEC>);
impl core::ops::Deref for W {
    type Target = hW<PIR_SPEC>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for W {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl From<hW<PIR_SPEC>> for W {
    #[inline(always)]
    fn from(writer: hW<PIR_SPEC>) -> Self {
        W(writer)
    }
}
#[doc = "Field `INTERVAL` reader - Polling interval"]
pub struct INTERVAL_R(FieldReader<u16, u16>);
impl INTERVAL_R {
    #[inline(always)]
    pub(crate) fn new(bits: u16) -> Self {
        INTERVAL_R(FieldReader::new(bits))
    }
}
impl core::ops::Deref for INTERVAL_R {
    type Target = FieldReader<u16, u16>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[doc = "Field `INTERVAL` writer - Polling interval"]
pub struct INTERVAL_W<'a> {
    w: &'a mut W,
}
impl<'a> INTERVAL_W<'a> {
    #[doc = r"Writes raw bits to the field"]
    #[inline(always)]
    pub unsafe fn bits(self, value: u16) -> &'a mut W {
        self.w.bits = (self.w.bits & !0xffff) | (value as u32 & 0xffff);
        self.w
    }
}
impl R {
    #[doc = "Bits 0:15 - Polling interval"]
    #[inline(always)]
    pub fn interval(&self) -> INTERVAL_R {
        INTERVAL_R::new((self.bits & 0xffff) as u16)
    }
}
impl W {
    #[doc = "Bits 0:15 - Polling interval"]
    #[inline(always)]
    pub fn interval(&mut self) -> INTERVAL_W<'_> {
        INTERVAL_W { w: self }
    }
    #[doc = "Writes raw bits to the register."]
    #[inline(always)]
    pub unsafe fn bits(&mut self, bits: u32) -> &mut Self {
        self.0.bits(bits);
        self
    }
}
#[doc = "polling interval register\n\nThis register you can [`read`](crate::generic::Reg::read), [`write_with_zero`](crate::generic::Reg::write_with_zero), [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`modify`](crate::generic::Reg::modify). See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [pir](index.html) module"]
pub struct PIR_SPEC;
impl RegisterSpec for PIR_SPEC {
    type Ux = u32;
}
#[doc = "`read()` method returns [pir::R](R) reader structure"]
impl Readable for PIR_SPEC {
    type Reader = R;
}
#[doc = "`write(|w| ..)` method takes [pir::W](W) writer structure"]
impl Writable for PIR_SPEC {
    type Writer = W;
}
#[doc = "`reset()` method sets PIR to value 0"]
impl Resettable for PIR_SPEC {
    #[inline(always)]
    fn reset_value() -> Self::Ux {
        0
    }
}

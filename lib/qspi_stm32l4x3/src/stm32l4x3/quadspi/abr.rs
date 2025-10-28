#![allow(non_camel_case_types)]

use crate::stm32l4x3::{
    field_reader::FieldReader,
    generics::{Readable, RegisterSpec, Resettable, Writable, R as hR, W as hW},
};

#[doc = "Register `ABR` reader"]
pub struct R(hR<ABR_SPEC>);
impl core::ops::Deref for R {
    type Target = hR<ABR_SPEC>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<hR<ABR_SPEC>> for R {
    #[inline(always)]
    fn from(reader: hR<ABR_SPEC>) -> Self {
        R(reader)
    }
}
#[doc = "Register `ABR` writer"]
pub struct W(hW<ABR_SPEC>);
impl core::ops::Deref for W {
    type Target = hW<ABR_SPEC>;
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
impl From<hW<ABR_SPEC>> for W {
    #[inline(always)]
    fn from(writer: hW<ABR_SPEC>) -> Self {
        W(writer)
    }
}
#[doc = "Field `ALTERNATE` reader - ALTERNATE"]
pub struct ALTERNATE_R(FieldReader<u32, u32>);
impl ALTERNATE_R {
    #[inline(always)]
    pub(crate) fn new(bits: u32) -> Self {
        ALTERNATE_R(FieldReader::new(bits))
    }
}
impl core::ops::Deref for ALTERNATE_R {
    type Target = FieldReader<u32, u32>;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[doc = "Field `ALTERNATE` writer - ALTERNATE"]
pub struct ALTERNATE_W<'a> {
    w: &'a mut W,
}
impl<'a> ALTERNATE_W<'a> {
    #[doc = r"Writes raw bits to the field"]
    #[inline(always)]
    pub unsafe fn bits(self, value: u32) -> &'a mut W {
        self.w.bits = value;
        self.w
    }
}
impl R {
    #[doc = "Bits 0:31 - ALTERNATE"]
    #[inline(always)]
    pub fn alternate(&self) -> ALTERNATE_R {
        ALTERNATE_R::new(self.bits)
    }
}
impl W {
    #[doc = "Bits 0:31 - ALTERNATE"]
    #[inline(always)]
    pub fn alternate(&mut self) -> ALTERNATE_W<'_> {
        ALTERNATE_W { w: self }
    }
    #[doc = "Writes raw bits to the register."]
    #[inline(always)]
    pub unsafe fn bits(&mut self, bits: u32) -> &mut Self {
        self.0.bits(bits);
        self
    }
}
#[doc = "ABR\n\nThis register you can [`read`](crate::generic::Reg::read), [`write_with_zero`](crate::generic::Reg::write_with_zero), [`reset`](crate::generic::Reg::reset), [`write`](crate::generic::Reg::write), [`modify`](crate::generic::Reg::modify). See [API](https://docs.rs/svd2rust/#read--modify--write-api).\n\nFor information about available fields see [abr](index.html) module"]
pub struct ABR_SPEC;
impl RegisterSpec for ABR_SPEC {
    type Ux = u32;
}
#[doc = "`read()` method returns [abr::R](R) reader structure"]
impl Readable for ABR_SPEC {
    type Reader = R;
}
#[doc = "`write(|w| ..)` method takes [abr::W](W) writer structure"]
impl Writable for ABR_SPEC {
    type Writer = W;
}
#[doc = "`reset()` method sets ABR to value 0"]
impl Resettable for ABR_SPEC {
    #[inline(always)]
    fn reset_value() -> Self::Ux {
        0
    }
}

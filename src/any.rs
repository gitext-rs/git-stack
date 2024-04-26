/// Mark a type as a [`Resource`]
pub trait ResourceTag: std::fmt::Debug + downcast_rs::DowncastSync + 'static {}

pub trait Resource: ResourceTag {
    fn clone_resource(&self) -> Box<dyn Resource>;
}

impl<T: ResourceTag + Clone> Resource for T {
    fn clone_resource(&self) -> Box<dyn Resource> {
        Box::new(self.clone())
    }
}

downcast_rs::impl_downcast!(sync Resource);

impl Clone for Box<dyn Resource> {
    fn clone(&self) -> Self {
        self.clone_resource()
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct BoxedResource(Box<dyn Resource>);

impl BoxedResource {
    pub(crate) fn as_ref<R: Resource>(&self) -> &R {
        self.0.as_any().downcast_ref::<R>().unwrap()
    }

    pub(crate) fn as_mut<R: Resource>(&mut self) -> &mut R {
        self.0.as_any_mut().downcast_mut::<R>().unwrap()
    }
}

impl std::fmt::Debug for BoxedResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}

#[derive(Clone)]
pub struct BoxedEntry {
    pub(crate) id: AnyId,
    pub(crate) value: BoxedResource,
}

impl BoxedEntry {
    pub(crate) fn new(r: impl Resource) -> Self {
        let id = AnyId::from(&r);
        let value = BoxedResource(Box::new(r));
        BoxedEntry { id, value }
    }
}

impl<R: Resource> From<R> for BoxedEntry {
    fn from(inner: R) -> Self {
        BoxedEntry::new(inner)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct AnyId {
    type_id: std::any::TypeId,
    #[cfg(debug_assertions)]
    type_name: &'static str,
}

impl AnyId {
    pub(crate) fn of<A: ?Sized + 'static>() -> Self {
        Self {
            type_id: std::any::TypeId::of::<A>(),
            #[cfg(debug_assertions)]
            type_name: std::any::type_name::<A>(),
        }
    }
}

impl PartialEq for AnyId {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for AnyId {}

impl PartialOrd for AnyId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnyId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_id.cmp(&other.type_id)
    }
}

impl std::hash::Hash for AnyId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

impl std::fmt::Debug for AnyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        #[cfg(not(debug_assertions))]
        {
            self.type_id.fmt(f)
        }
        #[cfg(debug_assertions)]
        {
            f.debug_tuple(self.type_name).field(&self.type_id).finish()
        }
    }
}

impl<'a, A: ?Sized + 'static> From<&'a A> for AnyId {
    fn from(_: &'a A) -> Self {
        Self::of::<A>()
    }
}

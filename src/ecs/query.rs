use std::{any::TypeId, collections::HashMap, marker::PhantomData};

use crate::ecs::{
    EntityID,
    component::Component,
    componentstore::{ComponentStore, ErasedComponentStore},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryAccessKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryAccess {
    pub type_id: TypeId,
    pub kind: QueryAccessKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryError {
    ConflictingAccess(TypeId),
}

pub fn validate_query_accesses(accesses: &[QueryAccess]) -> Result<(), QueryError> {
    let mut seen: HashMap<TypeId, QueryAccessKind> = HashMap::new();

    for access in accesses {
        match seen.get(&access.type_id) {
            Some(QueryAccessKind::Read) if access.kind == QueryAccessKind::Read => {}
            Some(_) => return Err(QueryError::ConflictingAccess(access.type_id)),
            None => {
                seen.insert(access.type_id, access.kind);
            }
        }
    }

    Ok(())
}

pub trait QueryParam<'a> {
    type Component: Component;
    type Item;

    fn access() -> QueryAccess;
    fn fetch(stores: &QueryStoreBorrow<'a>, entity: EntityID) -> Option<Self::Item>;
}

pub trait QueryTuple<'a> {
    type Item;

    fn accesses() -> Vec<QueryAccess>;
    fn fetch(stores: &QueryStoreBorrow<'a>, entity: EntityID) -> Option<Self::Item>;
}

pub enum BorrowedStore<'a> {
    Read(
        *const dyn ErasedComponentStore,
        PhantomData<&'a dyn ErasedComponentStore>,
    ),
    Write(
        *mut dyn ErasedComponentStore,
        PhantomData<&'a mut dyn ErasedComponentStore>,
    ),
}

pub struct QueryStoreBorrow<'a> {
    stores: HashMap<TypeId, BorrowedStore<'a>>,
}

impl<'a> QueryStoreBorrow<'a> {
    pub(crate) fn new() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }

    pub(crate) fn insert_read(&mut self, type_id: TypeId, store: *const dyn ErasedComponentStore) {
        self.stores
            .entry(type_id)
            .or_insert(BorrowedStore::Read(store, PhantomData));
    }

    pub(crate) fn insert_write(&mut self, type_id: TypeId, store: *mut dyn ErasedComponentStore) {
        self.stores
            .insert(type_id, BorrowedStore::Write(store, PhantomData));
    }

    fn read_store<T: Component>(&self) -> Option<&'a ComponentStore<T>> {
        match self.stores.get(&TypeId::of::<T>())? {
            BorrowedStore::Read(store, _) => {
                let store = unsafe { &*(*store) };
                store.as_any().downcast_ref::<ComponentStore<T>>()
            }
            BorrowedStore::Write(store, _) => {
                let store = unsafe { &*(*store) };
                store.as_any().downcast_ref::<ComponentStore<T>>()
            }
        }
    }

    fn write_store<T: Component>(&self) -> Option<&'a mut ComponentStore<T>> {
        match self.stores.get(&TypeId::of::<T>())? {
            BorrowedStore::Read(_, _) => None,
            BorrowedStore::Write(store, _) => {
                let store = unsafe { &mut *(*store) };
                store.as_any_mut().downcast_mut::<ComponentStore<T>>()
            }
        }
    }
}

pub struct QueryIter<'a, Q>
where
    Q: QueryTuple<'a>,
{
    entity_ids: Vec<EntityID>,
    current_index: usize,
    stores: QueryStoreBorrow<'a>,
    marker: PhantomData<Q>,
}

impl<'a, Q> QueryIter<'a, Q>
where
    Q: QueryTuple<'a>,
{
    pub(crate) fn new(entity_ids: Vec<EntityID>, stores: QueryStoreBorrow<'a>) -> Self {
        Self {
            entity_ids,
            current_index: 0,
            stores,
            marker: PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> &mut Self {
        self
    }
}

impl<'a, Q> Iterator for QueryIter<'a, Q>
where
    Q: QueryTuple<'a>,
{
    type Item = (EntityID, Q::Item);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(entity) = self.entity_ids.get(self.current_index).copied() {
            self.current_index += 1;
            if let Some(item) = Q::fetch(&self.stores, entity) {
                return Some((entity, item));
            }
        }

        None
    }
}

impl<'a, T: Component> QueryParam<'a> for &'a T {
    type Component = T;
    type Item = &'a T;

    fn access() -> QueryAccess {
        QueryAccess {
            type_id: TypeId::of::<T>(),
            kind: QueryAccessKind::Read,
        }
    }

    fn fetch(stores: &QueryStoreBorrow<'a>, entity: EntityID) -> Option<Self::Item> {
        stores.read_store::<T>()?.get(entity)
    }
}

impl<'a, T: Component> QueryParam<'a> for &'a mut T {
    type Component = T;
    type Item = &'a mut T;

    fn access() -> QueryAccess {
        QueryAccess {
            type_id: TypeId::of::<T>(),
            kind: QueryAccessKind::Write,
        }
    }

    fn fetch(stores: &QueryStoreBorrow<'a>, entity: EntityID) -> Option<Self::Item> {
        stores.write_store::<T>()?.get_mut(entity)
    }
}

macro_rules! impl_query_tuple {
    ($($name:ident),+) => {
        impl<'a, $($name),+> QueryTuple<'a> for ($($name,)+)
        where
            $($name: QueryParam<'a>,)+
        {
            type Item = ($(<$name as QueryParam<'a>>::Item,)+);

            fn accesses() -> Vec<QueryAccess> {
                vec![$(<$name as QueryParam<'a>>::access(),)+]
            }

            fn fetch(
                stores: &QueryStoreBorrow<'a>,
                entity: EntityID,
            ) -> Option<Self::Item> {
                Some(($(<$name as QueryParam<'a>>::fetch(stores, entity)?,)+))
            }
        }
    };
}

impl_query_tuple!(A);
impl_query_tuple!(A, B);
impl_query_tuple!(A, B, C);
impl_query_tuple!(A, B, C, D);
impl_query_tuple!(A, B, C, D, E);
impl_query_tuple!(A, B, C, D, E, F);
impl_query_tuple!(A, B, C, D, E, F, G);
impl_query_tuple!(A, B, C, D, E, F, G, H);
impl_query_tuple!(A, B, C, D, E, F, G, H, I);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_query_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Position {
        x: i32,
    }

    #[derive(Debug, PartialEq)]
    struct Velocity {
        dx: i32,
    }

    #[test]
    fn query_param_accesses_track_component_type_and_access_kind() {
        let read_access = <&Position as QueryParam>::access();
        let write_access = <&mut Velocity as QueryParam>::access();

        assert_eq!(read_access.type_id, TypeId::of::<Position>());
        assert_eq!(read_access.kind, QueryAccessKind::Read);
        assert_eq!(write_access.type_id, TypeId::of::<Velocity>());
        assert_eq!(write_access.kind, QueryAccessKind::Write);
    }

    #[test]
    fn query_tuple_accesses_preserve_tuple_order() {
        let accesses = <(&mut Position, &Velocity) as QueryTuple>::accesses();

        assert_eq!(accesses.len(), 2);
        assert_eq!(accesses[0].type_id, TypeId::of::<Position>());
        assert_eq!(accesses[0].kind, QueryAccessKind::Write);
        assert_eq!(accesses[1].type_id, TypeId::of::<Velocity>());
        assert_eq!(accesses[1].kind, QueryAccessKind::Read);
    }

    #[test]
    fn validate_query_accesses_allows_repeated_reads() {
        let accesses = <(&Position, &Position) as QueryTuple>::accesses();

        assert_eq!(validate_query_accesses(&accesses), Ok(()));
    }

    #[test]
    fn validate_query_accesses_rejects_conflicting_access() {
        let accesses = <(&Position, &mut Position) as QueryTuple>::accesses();

        assert!(matches!(
            validate_query_accesses(&accesses),
            Err(QueryError::ConflictingAccess(type_id)) if type_id == TypeId::of::<Position>()
        ));
    }

    #[test]
    fn fetch_reads_and_writes_components_from_borrowed_stores() {
        let entity = 7;
        let mut position_store = ComponentStore::new();
        let mut velocity_store = ComponentStore::new();
        position_store.insert(entity, Position { x: 1 });
        velocity_store.insert(entity, Velocity { dx: 2 });

        let mut stores = QueryStoreBorrow::new();
        stores.insert_write(
            TypeId::of::<Position>(),
            &mut position_store as &mut dyn ErasedComponentStore,
        );
        stores.insert_read(
            TypeId::of::<Velocity>(),
            &velocity_store as &dyn ErasedComponentStore,
        );

        let (position, velocity) =
            <(&mut Position, &Velocity) as QueryTuple>::fetch(&stores, entity).unwrap();
        position.x += velocity.dx;

        assert_eq!(position_store.get(entity), Some(&Position { x: 3 }));
    }

    #[test]
    fn query_iter_skips_entities_that_do_not_fetch() {
        let matching_entity = 2;
        let missing_entity = 9;
        let mut position_store = ComponentStore::new();
        position_store.insert(matching_entity, Position { x: 5 });

        let mut stores = QueryStoreBorrow::new();
        stores.insert_read(
            TypeId::of::<Position>(),
            &position_store as &dyn ErasedComponentStore,
        );

        let values: Vec<_> =
            QueryIter::<(&Position,)>::new(vec![missing_entity, matching_entity], stores)
                .map(|(_, components)| components.0.x)
                .collect();

        assert_eq!(values, vec![5]);
    }
}

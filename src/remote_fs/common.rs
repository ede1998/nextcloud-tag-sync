use crate::newtype;
use futures::Future;

newtype!(TagId, u64);
newtype!(FileId, u64);

pub struct LimitedConcurrency<Iter> {
    elements: Iter,
    max_concurrent_requests: usize,
}

impl<Iter> LimitedConcurrency<Iter> {
    pub const fn new(elements: Iter, max_concurrent_requests: usize) -> Self {
        Self {
            elements,
            max_concurrent_requests,
        }
    }

    pub const fn transform<F, Fut>(self, element_action: F) -> TransformElements<Iter, F>
    where
        Iter: IntoIterator,
        F: Fn(Iter::Item) -> Fut,
        Fut: Future,
    {
        TransformElements {
            base: self,
            element_action,
        }
    }
}

pub struct TransformElements<Iter, EAction> {
    base: LimitedConcurrency<Iter>,
    element_action: EAction,
}

impl<Iter, EAction> TransformElements<Iter, EAction> {
    pub(crate) const fn aggregate<AAction>(
        self,
        aggregate_action: AAction,
    ) -> AggregateElements<Iter, EAction, AAction> {
        AggregateElements {
            base: self,
            aggregate_action,
        }
    }

    pub(crate) async fn collect_err<Res, Fut>(self) -> Res
    where
        Res: Extend<Iter::Item> + Default,
        Iter: IntoIterator,
        EAction: Fn(Iter::Item) -> Fut,
        Fut: Future<Output = Result<(), Iter::Item>>,
    {
        use futures::StreamExt;
        futures::stream::iter(self.base.elements)
            .map(self.element_action)
            .buffer_unordered(self.base.max_concurrent_requests)
            .filter_map(|item| futures::future::ready(item.err()))
            .collect()
            .await
    }
}

pub struct AggregateElements<Iter, EAction, AAction> {
    pub base: TransformElements<Iter, EAction>,
    pub aggregate_action: AAction,
}

impl<Iter, EAction, AAction> AggregateElements<Iter, EAction, AAction> {
    pub(crate) async fn collect<Res, EFut>(self) -> Res
    where
        Res: Default,
        AAction: Fn(&mut Res, EFut::Output),
        Iter: IntoIterator,
        EAction: Fn(Iter::Item) -> EFut,
        EFut: Future,
    {
        use futures::StreamExt;
        futures::stream::iter(self.base.base.elements)
            .map(self.base.element_action)
            .buffer_unordered(self.base.base.max_concurrent_requests)
            .fold(Res::default(), |mut res, temp| {
                (self.aggregate_action)(&mut res, temp);
                futures::future::ready(res)
            })
            .await
    }
}
